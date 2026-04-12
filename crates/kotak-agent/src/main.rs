use anyhow::Result;
use kotak_agent::{
    filesystem::FilesystemManager,
    network::IpamAllocator,
    sandbox::{Sandbox, SandboxConfig},
    snapshot::SnapshotStore,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let id = "sandbox-001";
    let ipam = IpamAllocator::new();
    let store = SnapshotStore::new();
    let fs = FilesystemManager::new("/home/naufal/kotak/firecracker-local/rootfs.ext4");

    let config = SandboxConfig {
        kernel_path: "/home/naufal/kotak/firecracker-local/vmlinux-6.1.155.bin".to_string(),
        mac: "AA:FC:00:00:00:01".to_string(),
        guest_cid: 3,
    };

    let sandbox = Sandbox::create(id, &ipam, fs, config).await?;

    let response = sandbox.exec("uname -a").await?;
    tracing::info!("stdout: {}", response.stdout);

    sandbox.hibernate(&store).await?;
    sandbox.destroy(&ipam).await?;

    Ok(())
}
