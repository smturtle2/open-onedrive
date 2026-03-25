use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, ReplyOpen,
    Request,
};
use libc::{EISDIR, ENOENT};
use openonedrive_ipc_types::{AvailabilityState, ItemKind};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::io;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1);
const ROOT_INO: u64 = 1;

#[derive(Debug, Clone)]
pub struct VirtualEntry {
    pub path: String,
    pub kind: ItemKind,
    pub availability: AvailabilityState,
    pub pinned: bool,
    pub size: u64,
    pub modified_unix: i64,
    pub content_stub: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotHandle(Arc<RwLock<TreeState>>);

pub trait ContentProvider: Send + Sync {
    fn read_all(&self, path: &str) -> io::Result<Vec<u8>>;
}

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

pub struct OpenOneDriveFs {
    snapshot: SnapshotHandle,
    content_provider: Option<Arc<dyn ContentProvider>>,
}

impl OpenOneDriveFs {
    pub fn new(
        snapshot: SnapshotHandle,
        content_provider: Option<Arc<dyn ContentProvider>>,
    ) -> Self {
        Self {
            snapshot,
            content_provider,
        }
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
        if node.kind != ItemKind::Directory {
            reply.error(ENOENT);
            return;
        }

        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".into()),
            (node.parent.unwrap_or(ROOT_INO), FileType::Directory, "..".into()),
        ];
        for child in state.children.get(&ino).into_iter().flatten() {
            if let Some(child_node) = state.by_ino.get(child) {
                entries.push((child_node.attr.ino, to_file_type(child_node.kind), child_node.name.clone()));
            }
        }

        for (index, entry) in entries.into_iter().enumerate().skip(offset.max(0) as usize) {
            let full = reply.add(entry.0, (index + 1) as i64, entry.1, entry.2);
            if full {
                break;
            }
        }

        reply.ok();
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let state = self.snapshot.read();
        match state.by_ino.get(&ino) {
            Some(node) if node.kind == ItemKind::Directory => reply.error(EISDIR),
            Some(_) => reply.opened(ino, flags as u32),
            None => reply.error(ENOENT),
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let state = self.snapshot.read();
        let Some(node) = state.by_ino.get(&ino) else {
            reply.error(ENOENT);
            return;
        };
        if node.kind == ItemKind::Directory {
            reply.error(EISDIR);
            return;
        }

        let content = self
            .content_provider
            .as_ref()
            .and_then(|provider| provider.read_all(&node.path).ok())
            .unwrap_or_else(|| node.content.clone());
        let start = offset.max(0) as usize;
        let end = (start + size as usize).min(content.len());
        if start >= content.len() {
            reply.data(&[]);
            return;
        }
        reply.data(&content[start..end]);
    }
}

#[derive(Debug, Clone)]
struct Node {
    attr: FileAttr,
    parent: Option<u64>,
    path: String,
    name: String,
    kind: ItemKind,
    content: Vec<u8>,
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
            attr: make_attr(ROOT_INO, ItemKind::Directory, 0, now_unix()),
            parent: None,
            path: "/".into(),
            name: "/".into(),
            kind: ItemKind::Directory,
            content: Vec::new(),
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
            let path = normalize_path(&entry.path);
            let mut current_parent = ROOT_INO;
            let mut current_path = String::new();

            for (index, segment) in path.trim_matches('/').split('/').enumerate() {
                current_path.push('/');
                current_path.push_str(segment);

                if tree.path_to_ino.contains_key(&current_path) {
                    current_parent = tree.path_to_ino[&current_path];
                    continue;
                }

                let is_leaf = index == path.trim_matches('/').split('/').count() - 1;
                let kind = if is_leaf { entry.kind } else { ItemKind::Directory };
                let content = if kind == ItemKind::File {
                    build_content(entry)
                } else {
                    Vec::new()
                };
                let size = if kind == ItemKind::File {
                    entry.size.max(content.len() as u64)
                } else {
                    0
                };

                let node = Node {
                    attr: make_attr(next_ino, kind, size, entry.modified_unix),
                    parent: Some(current_parent),
                    path: current_path.clone(),
                    name: segment.to_string(),
                    kind,
                    content,
                };
                tree.children.entry(current_parent).or_default().push(next_ino);
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

fn build_content(entry: &VirtualEntry) -> Vec<u8> {
    if let Some(content) = &entry.content_stub {
        return content.as_bytes().to_vec();
    }

    format!(
        "open-onedrive placeholder\n\npath: {}\navailability: {:?}\npinned: {}\n",
        entry.path, entry.availability, entry.pinned
    )
    .into_bytes()
}

fn normalize_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn make_attr(ino: u64, kind: ItemKind, size: u64, modified_unix: i64) -> FileAttr {
    let modified = UNIX_EPOCH + Duration::from_secs(modified_unix.max(0) as u64);
    FileAttr {
        ino,
        size,
        blocks: 1,
        atime: modified,
        mtime: modified,
        ctime: modified,
        crtime: modified,
        kind: to_file_type(kind),
        perm: if kind == ItemKind::Directory { 0o755 } else { 0o444 },
        nlink: if kind == ItemKind::Directory { 2 } else { 1 },
        uid: 0,
        gid: 0,
        rdev: 0,
        flags: 0,
        blksize: 4096,
    }
}

fn to_file_type(kind: ItemKind) -> FileType {
    match kind {
        ItemKind::Directory => FileType::Directory,
        ItemKind::File => FileType::RegularFile,
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}
