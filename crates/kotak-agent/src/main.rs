use anyhow::Result;
use kotak_agent::{
    filesystem::FilesystemManager,
    network::IpamAllocator,
    sandbox::{Sandbox, SandboxConfig, resume},
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

    let sandbox = Sandbox::create(id, &ipam, fs, &config).await?;
    let response = sandbox.exec("uname -a").await?;
    tracing::info!("stdout: {}", response.stdout);

    let response = sandbox
        .exec("echo 'kotak works' > /root/testfile.txt && cat /root/testfile.txt")
        .await?;
    tracing::info!("created: {}", response.stdout.trim());

    sandbox.hibernate(&store).await?;
    sandbox.destroy(&ipam).await?;

    tracing::info!("here");
    let fs = FilesystemManager::new("/home/naufal/kotak/firecracker-local/rootfs.ext4");
    let sandbox = resume(id, &ipam, fs, &store, &config).await?;
    let response = sandbox.exec("uname -a").await?;
    tracing::info!("STDOUTT HERE: {}", response.stdout);

    let response = sandbox.exec("cat /root/testfile.txt").await?;
    tracing::info!("HEREEEEEE: {}", response.stdout.trim());

    sandbox.destroy(&ipam).await?;

    Ok(())
}
