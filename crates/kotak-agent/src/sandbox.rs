use crate::{
    filesystem::FilesystemManager,
    firecracker::{
        client::{ExecResponse, FirecrackerClient, ResolvedConfig},
        process::FirecrackerProcess,
    },
    network::{IpamAllocator, TapNetwork, setup_tap, teardown_tap},
    snapshot::SnapshotStore,
};
use anyhow::Result;
use std::path::PathBuf;

pub struct SandboxConfig {
    pub kernel_path: String,
    pub mac: String,
    pub guest_cid: u32,
}

pub struct Sandbox {
    pub id: String,
    process: FirecrackerProcess,
    client: FirecrackerClient,
    net: TapNetwork,
    fs: FilesystemManager,
}

impl Sandbox {
    pub async fn create(
        id: &str,
        ipam: &IpamAllocator,
        fs: FilesystemManager,
        config: SandboxConfig,
    ) -> Result<Self> {
        let rootfs_path = fs.prepare(id).await?;
        let net = ipam.allocate(id).await?;
        setup_tap(&net).await?;

        let process = FirecrackerProcess::spawn(id).await?;
        let client = FirecrackerClient::new(&process.socket_path);

        let resolved = ResolvedConfig {
            kernel_path: &config.kernel_path,
            rootfs_path: rootfs_path.to_str().unwrap(),
            mac: &config.mac,
            guest_cid: config.guest_cid,
            tap_name: &net.tap_name,
            guest_ip: &net.guest_ip,
            gateway_ip: &net.host_ip,
            vsock_path: &process.vsock_path,
        };

        client.launch(&resolved).await?;

        Ok(Self {
            id: id.to_string(),
            process,
            client,
            net,
            fs,
        })
    }

    pub async fn exec(&self, command: &str) -> Result<ExecResponse> {
        self.client.exec(&self.process.vsock_path, command).await
    }

    pub async fn hibernate(&self, store: &SnapshotStore) -> Result<()> {
        self.client.stop().await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        store
            .snapshot_filesystem(&self.id, &self.fs.rootfs_path(&self.id))
            .await?;
        Ok(())
    }

    pub async fn destroy(self, ipam: &IpamAllocator) -> Result<()> {
        teardown_tap(&self.net).await?;
        ipam.release(self.net.slot).await;
        self.fs.teardown(&self.id).await?;
        Ok(())
    }
}

// pub async fn resume(
//     id: &str,
//     ipam: &IpamAllocator,
//     store: &SnapshotStore,
//     config: SandboxConfig,
// ) -> Result<Sandbox> {
//     let upper_dir = upper_dir_for(id);
//     store.restore_filesystem(id, &upper_dir).await?;
//     Sandbox::create(id, ipam, config).await
// }

fn upper_dir_for(id: &str) -> PathBuf {
    PathBuf::from(format!("/var/lib/kotak/sandboxes/{}/upper", id))
}
