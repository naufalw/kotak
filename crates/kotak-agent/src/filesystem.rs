use std::path::PathBuf;

use anyhow::Result;

use crate::cmd::run_cmd;

pub struct FilesystemManager {
    base_rootfs: PathBuf,
    sandboxes_dir: PathBuf,
}

impl FilesystemManager {
    pub fn new(base_rootfs: impl Into<PathBuf>) -> Self {
        Self {
            base_rootfs: base_rootfs.into(),
            sandboxes_dir: PathBuf::from("/var/lib/kotak/sandboxes"),
        }
    }

    pub async fn prepare(&self, id: &str) -> Result<PathBuf> {
        let sandbox_dir = self.sandboxes_dir.join(id);
        tokio::fs::create_dir_all(&sandbox_dir).await?;

        let dest = sandbox_dir.join("rootfs.ext4");

        run_cmd(&[
            "cp",
            "--sparse=always",
            self.base_rootfs.to_str().unwrap(),
            dest.to_str().unwrap(),
        ])
        .await?;

        Ok(dest)
    }

    pub async fn teardown(&self, id: &str) -> Result<()> {
        let sandbox_dir = self.sandboxes_dir.join(id);
        tokio::fs::remove_dir_all(&sandbox_dir).await?;
        Ok(())
    }

    pub fn rootfs_path(&self, id: &str) -> PathBuf {
        self.sandboxes_dir.join(id).join("rootfs.ext4")
    }
}
