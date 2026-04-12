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

pub struct SandboxConfig {
    pub kernel_path: String,
    pub guest_cid: u32,
}

pub struct Sandbox {
    pub id: String,
    pub net: TapNetwork,
    process: FirecrackerProcess,
    client: FirecrackerClient,
    fs: FilesystemManager,
}

impl Sandbox {
    pub async fn create(
        id: &str,
        ipam: &IpamAllocator,
        fs: FilesystemManager,
        config: &SandboxConfig,
    ) -> Result<Self> {
        let rootfs_path = fs.prepare(id).await?;
        let net = ipam.allocate(id).await?;
        let mac = format!("AA:FC:00:00:{:02X}:{:02X}", net.slot >> 8, net.slot & 0xff);
        setup_tap(&net).await?;

        let process = FirecrackerProcess::spawn(id).await?;
        let client = FirecrackerClient::new(&process.socket_path);

        let resolved = ResolvedConfig {
            kernel_path: &config.kernel_path,
            rootfs_path: rootfs_path.to_str().unwrap(),
            mac: &mac,
            guest_cid: config.guest_cid,
            tap_name: &net.tap_name,
            guest_ip: &net.guest_ip,
            gateway_ip: &net.host_ip,
            vsock_path: &process.vsock_path,
        };

        client.launch(&resolved).await?;
        client
            .exec(
                &process.vsock_path,
                "rm -f /etc/ssh/ssh_host_* && ssh-keygen -A && rc-service sshd restart",
            )
            .await?;

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

pub async fn resume(
    id: &str,
    ipam: &IpamAllocator,
    fs: FilesystemManager,
    store: &SnapshotStore,
    config: &SandboxConfig,
) -> Result<Sandbox> {
    let rootfs_path = fs.prepare_empty(id).await?;

    store.restore_filesystem(id, &rootfs_path).await?;

    let net = ipam.allocate(id).await?;
    let mac = format!("AA:FC:00:00:{:02X}:{:02X}", net.slot >> 8, net.slot & 0xff);
    setup_tap(&net).await?;

    let process = FirecrackerProcess::spawn(id).await?;
    let client = FirecrackerClient::new(&process.socket_path);

    let resolved = ResolvedConfig {
        kernel_path: &config.kernel_path,
        rootfs_path: rootfs_path.to_str().unwrap(),
        mac: &mac,
        guest_cid: config.guest_cid,
        tap_name: &net.tap_name,
        guest_ip: &net.guest_ip,
        gateway_ip: &net.host_ip,
        vsock_path: &process.vsock_path,
    };

    client.launch(&resolved).await?;

    Ok(Sandbox {
        id: id.to_string(),
        process,
        client,
        net,
        fs,
    })
}
