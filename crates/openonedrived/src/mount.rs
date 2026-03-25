use anyhow::{Context, Result};
use fuser::{BackgroundSession, MountOption};
use openonedrive_vfs::{ContentProvider, OpenOneDriveFs, SnapshotHandle, VirtualEntry};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct MountController {
    snapshot: SnapshotHandle,
    content_provider: Option<Arc<dyn ContentProvider>>,
    session: Option<BackgroundSession>,
    mount_path: Option<PathBuf>,
}

impl MountController {
    pub fn new(snapshot: SnapshotHandle, content_provider: Option<Arc<dyn ContentProvider>>) -> Self {
        Self {
            snapshot,
            content_provider,
            session: None,
            mount_path: None,
        }
    }

    pub fn rebuild(&self, entries: &[VirtualEntry]) {
        self.snapshot.rebuild(entries);
    }

    pub fn mount(&mut self, path: &Path) -> Result<()> {
        self.unmount();
        let options = vec![
            MountOption::FSName("open-onedrive".into()),
            MountOption::AutoUnmount,
            MountOption::DefaultPermissions,
            MountOption::NoAtime,
            MountOption::RO,
        ];
        let filesystem =
            OpenOneDriveFs::new(self.snapshot.clone(), self.content_provider.clone());
        let session = fuser::spawn_mount2(filesystem, path, &options)
            .with_context(|| format!("unable to mount FUSE filesystem at {}", path.display()))?;
        self.session = Some(session);
        self.mount_path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn unmount(&mut self) {
        self.session.take();
        self.mount_path = None;
    }
}
