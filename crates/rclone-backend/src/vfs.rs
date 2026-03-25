use fuser::{
    BackgroundSession, FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyCreate,
    ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request,
};
use libc::{EEXIST, EINVAL, EIO, EISDIR, ENOENT, ENOTDIR};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1);
const ROOT_INO: u64 = 1;

#[derive(Debug, Clone)]
pub struct VirtualEntry {
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_unix: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct OpenRequest {
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
}

pub trait Provider: Send + Sync {
    fn snapshot_entries(&self) -> io::Result<Vec<VirtualEntry>>;
    fn open_file(&self, path: &str, request: OpenRequest) -> io::Result<File>;
    fn create_dir(&self, path: &str) -> io::Result<()>;
    fn remove_file(&self, path: &str) -> io::Result<()>;
    fn remove_dir(&self, path: &str) -> io::Result<()>;
    fn rename_path(&self, from: &str, to: &str) -> io::Result<()>;
    fn set_len(&self, path: &str, size: u64) -> io::Result<()>;
    fn finish_write(&self, path: &str) -> io::Result<()>;
}

#[derive(Debug, Clone)]
pub struct SnapshotHandle(Arc<RwLock<TreeState>>);

impl Default for SnapshotHandle {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(TreeState::empty())))
    }
}

impl SnapshotHandle {
    pub fn rebuild(&self, items: &[VirtualEntry]) {
        let mut state = self.0.write().expect("vfs tree poisoned");
        *state = TreeState::build(items);
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, TreeState> {
        self.0.read().expect("vfs tree poisoned")
    }
}

struct OpenHandle {
    path: String,
    file: File,
    writable: bool,
}

pub struct OpenOneDriveFs {
    snapshot: SnapshotHandle,
    provider: Arc<dyn Provider>,
    handles: std::sync::Mutex<HashMap<u64, OpenHandle>>,
    next_fh: AtomicU64,
}

impl OpenOneDriveFs {
    pub fn new(snapshot: SnapshotHandle, provider: Arc<dyn Provider>) -> Self {
        Self {
            snapshot,
            provider,
            handles: std::sync::Mutex::new(HashMap::new()),
            next_fh: AtomicU64::new(2),
        }
    }

    pub fn mount_options() -> Vec<MountOption> {
        vec![
            MountOption::FSName("openonedrive".to_string()),
            MountOption::AutoUnmount,
            MountOption::DefaultPermissions,
        ]
    }

    pub fn mount(
        snapshot: SnapshotHandle,
        provider: Arc<dyn Provider>,
        mountpoint: &Path,
    ) -> io::Result<BackgroundSession> {
        fuser::spawn_mount2(
            Self::new(snapshot, provider),
            mountpoint,
            &Self::mount_options(),
        )
    }

    fn refresh_snapshot(&self) -> io::Result<()> {
        let entries = self.provider.snapshot_entries()?;
        self.snapshot.rebuild(&entries);
        Ok(())
    }

    fn path_for_child(&self, parent: u64, name: &OsStr) -> Result<String, i32> {
        let Some(name) = name.to_str() else {
            return Err(EINVAL);
        };
        let state = self.snapshot.read();
        let Some(node) = state.by_ino.get(&parent) else {
            return Err(ENOENT);
        };
        if !node.is_dir {
            return Err(ENOTDIR);
        }
        Ok(join_path(&node.path, name))
    }

    fn attr_for_path(&self, path: &str) -> Option<FileAttr> {
        let state = self.snapshot.read();
        state
            .path_to_ino
            .get(path)
            .and_then(|ino| state.by_ino.get(ino))
            .map(|node| node.attr)
    }

    fn open_handle(&self, path: String, file: File, writable: bool) -> u64 {
        let fh = self.next_fh.fetch_add(1, Ordering::Relaxed);
        self.handles.lock().expect("open handles poisoned").insert(
            fh,
            OpenHandle {
                path,
                file,
                writable,
            },
        );
        fh
    }
}

impl Filesystem for OpenOneDriveFs {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let state = self.snapshot.read();
        let Some(name) = name.to_str() else {
            reply.error(ENOENT);
            return;
        };

        if let Some(node) = state.lookup_child(parent, name) {
            reply.entry(&TTL, &node.attr, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        let state = self.snapshot.read();
        if let Some(node) = state.by_ino.get(&ino) {
            reply.attr(&TTL, &node.attr);
        } else {
            reply.error(ENOENT);
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let path = {
            let state = self.snapshot.read();
            let Some(node) = state.by_ino.get(&ino) else {
                reply.error(ENOENT);
                return;
            };
            if node.is_dir {
                reply.attr(&TTL, &node.attr);
                return;
            }
            node.path.clone()
        };

        if let Some(size) = size {
            if self.provider.set_len(&path, size).is_err() || self.refresh_snapshot().is_err() {
                reply.error(EIO);
                return;
            }
        }

        match self.attr_for_path(&path) {
            Some(attr) => reply.attr(&TTL, &attr),
            None => reply.error(ENOENT),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let state = self.snapshot.read();
        let Some(node) = state.by_ino.get(&ino) else {
            reply.error(ENOENT);
            return;
        };
        if !node.is_dir {
            reply.error(ENOTDIR);
            return;
        }

        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".into()),
            (
                node.parent.unwrap_or(ROOT_INO),
                FileType::Directory,
                "..".into(),
            ),
        ];
        if let Some(children) = state.children.get(&ino) {
            for child in children {
                if let Some(child_node) = state.by_ino.get(child) {
                    entries.push((
                        child_node.attr.ino,
                        file_type(child_node.is_dir),
                        child_node.name.clone(),
                    ));
                }
            }
        }

        let offset = offset.max(0) as usize;
        for (index, entry) in entries.into_iter().enumerate().skip(offset) {
            if reply.add(entry.0, (index + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let path = match self.path_for_child(parent, name) {
            Ok(path) => path,
            Err(code) => {
                reply.error(code);
                return;
            }
        };

        if self.provider.create_dir(&path).is_err() || self.refresh_snapshot().is_err() {
            reply.error(EIO);
            return;
        }

        match self.attr_for_path(&path) {
            Some(attr) => reply.entry(&TTL, &attr, 0),
            None => reply.error(EIO),
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let path = match self.path_for_child(parent, name) {
            Ok(path) => path,
            Err(code) => {
                reply.error(code);
                return;
            }
        };

        if self.provider.remove_file(&path).is_err() || self.refresh_snapshot().is_err() {
            reply.error(EIO);
            return;
        }
        reply.ok();
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let path = match self.path_for_child(parent, name) {
            Ok(path) => path,
            Err(code) => {
                reply.error(code);
                return;
            }
        };

        if self.provider.remove_dir(&path).is_err() || self.refresh_snapshot().is_err() {
            reply.error(EIO);
            return;
        }
        reply.ok();
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let from = match self.path_for_child(parent, name) {
            Ok(path) => path,
            Err(code) => {
                reply.error(code);
                return;
            }
        };
        let to = match self.path_for_child(newparent, newname) {
            Ok(path) => path,
            Err(code) => {
                reply.error(code);
                return;
            }
        };

        if self.provider.rename_path(&from, &to).is_err() || self.refresh_snapshot().is_err() {
            reply.error(EIO);
            return;
        }
        reply.ok();
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let path = {
            let state = self.snapshot.read();
            let Some(node) = state.by_ino.get(&ino) else {
                reply.error(ENOENT);
                return;
            };
            if node.is_dir {
                reply.error(EISDIR);
                return;
            }
            node.path.clone()
        };

        let request = OpenRequest {
            write: (flags & libc::O_ACCMODE) != libc::O_RDONLY,
            create: false,
            truncate: (flags & libc::O_TRUNC) != 0,
        };
        match self.provider.open_file(&path, request) {
            Ok(file) => {
                let fh = self.open_handle(path, file, request.write);
                reply.opened(fh, 0);
            }
            Err(_) => reply.error(EIO),
        }
    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let path = match self.path_for_child(parent, name) {
            Ok(path) => path,
            Err(code) => {
                reply.error(code);
                return;
            }
        };
        if self.attr_for_path(&path).is_some() {
            reply.error(EEXIST);
            return;
        }

        let request = OpenRequest {
            write: true,
            create: true,
            truncate: true,
        };
        match self.provider.open_file(&path, request) {
            Ok(file) => {
                if self.refresh_snapshot().is_err() {
                    reply.error(EIO);
                    return;
                }
                let Some(attr) = self.attr_for_path(&path) else {
                    reply.error(EIO);
                    return;
                };
                let fh = self.open_handle(path, file, true);
                reply.created(&TTL, &attr, 0, fh, flags as u32);
            }
            Err(_) => reply.error(EIO),
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        use std::os::unix::fs::FileExt;

        let handles = self.handles.lock().expect("open handles poisoned");
        let Some(handle) = handles.get(&fh) else {
            reply.error(ENOENT);
            return;
        };
        let offset = offset.max(0) as u64;
        let mut buffer = vec![0_u8; size as usize];
        match handle.file.read_at(&mut buffer, offset) {
            Ok(bytes) => reply.data(&buffer[..bytes]),
            Err(_) => reply.error(EIO),
        }
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        use std::os::unix::fs::FileExt;

        let mut handles = self.handles.lock().expect("open handles poisoned");
        let Some(handle) = handles.get_mut(&fh) else {
            reply.error(ENOENT);
            return;
        };
        if !handle.writable {
            reply.error(EINVAL);
            return;
        }
        let offset = offset.max(0) as u64;
        match handle.file.write_at(data, offset) {
            Ok(bytes) => reply.written(bytes as u32),
            Err(_) => reply.error(EIO),
        }
    }

    fn fsync(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
        let handles = self.handles.lock().expect("open handles poisoned");
        let Some(handle) = handles.get(&fh) else {
            reply.error(ENOENT);
            return;
        };
        if handle.file.sync_all().is_err() {
            reply.error(EIO);
            return;
        }
        reply.ok();
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        _ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let handle = self
            .handles
            .lock()
            .expect("open handles poisoned")
            .remove(&fh);
        let Some(handle) = handle else {
            reply.ok();
            return;
        };

        if handle.file.sync_all().is_err() {
            reply.error(EIO);
            return;
        }
        if handle.writable
            && (self.provider.finish_write(&handle.path).is_err()
                || self.refresh_snapshot().is_err())
        {
            reply.error(EIO);
            return;
        }
        reply.ok();
    }
}

#[derive(Debug, Clone)]
struct Node {
    attr: FileAttr,
    parent: Option<u64>,
    path: String,
    name: String,
    is_dir: bool,
}

#[derive(Debug, Default, Clone)]
struct TreeState {
    by_ino: BTreeMap<u64, Node>,
    path_to_ino: HashMap<String, u64>,
    children: BTreeMap<u64, Vec<u64>>,
}

impl TreeState {
    fn empty() -> Self {
        let root = Node {
            attr: make_attr(ROOT_INO, true, 0, now_unix()),
            parent: None,
            path: "/".into(),
            name: "/".into(),
            is_dir: true,
        };
        let mut by_ino = BTreeMap::new();
        by_ino.insert(ROOT_INO, root);

        let mut path_to_ino = HashMap::new();
        path_to_ino.insert("/".into(), ROOT_INO);

        Self {
            by_ino,
            path_to_ino,
            children: BTreeMap::new(),
        }
    }

    fn build(entries: &[VirtualEntry]) -> Self {
        let mut tree = Self::empty();
        let mut next_ino = ROOT_INO + 1;

        for entry in entries {
            let normalized = normalize_path(&entry.path);
            let segments = path_segments(&normalized);
            let mut current_parent = ROOT_INO;
            let mut current_path = String::new();

            for (index, segment) in segments.iter().enumerate() {
                current_path.push('/');
                current_path.push_str(segment);

                if let Some(existing) = tree.path_to_ino.get(&current_path).copied() {
                    current_parent = existing;
                    continue;
                }

                let is_leaf = index + 1 == segments.len();
                let is_dir = if is_leaf { entry.is_dir } else { true };
                let size = if is_leaf && !entry.is_dir {
                    entry.size_bytes
                } else {
                    0
                };
                let modified = if is_leaf {
                    entry.modified_unix
                } else {
                    entry.modified_unix
                };

                let node = Node {
                    attr: make_attr(next_ino, is_dir, size, modified),
                    parent: Some(current_parent),
                    path: current_path.clone(),
                    name: segment.to_string(),
                    is_dir,
                };
                tree.children
                    .entry(current_parent)
                    .or_default()
                    .push(next_ino);
                tree.path_to_ino.insert(current_path.clone(), next_ino);
                tree.by_ino.insert(next_ino, node);
                current_parent = next_ino;
                next_ino += 1;
            }
        }

        tree
    }

    fn lookup_child(&self, parent: u64, name: &str) -> Option<&Node> {
        self.children.get(&parent).and_then(|children| {
            children
                .iter()
                .filter_map(|ino| self.by_ino.get(ino))
                .find(|node| node.name == name)
        })
    }
}

fn join_path(parent: &str, child: &str) -> String {
    if parent == "/" {
        format!("/{child}")
    } else {
        format!("{parent}/{child}")
    }
}

fn normalize_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn path_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn make_attr(ino: u64, is_dir: bool, size: u64, modified_unix: u64) -> FileAttr {
    let modified = UNIX_EPOCH + Duration::from_secs(modified_unix);
    FileAttr {
        ino,
        size,
        blocks: size.div_ceil(512),
        atime: modified,
        mtime: modified,
        ctime: modified,
        crtime: modified,
        kind: file_type(is_dir),
        perm: if is_dir { 0o755 } else { 0o644 },
        nlink: if is_dir { 2 } else { 1 },
        uid: unsafe { libc::geteuid() },
        gid: unsafe { libc::getegid() },
        rdev: 0,
        blksize: 4096,
        flags: 0,
    }
}

fn file_type(is_dir: bool) -> FileType {
    if is_dir {
        FileType::Directory
    } else {
        FileType::RegularFile
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
