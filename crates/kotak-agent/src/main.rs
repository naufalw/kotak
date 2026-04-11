use anyhow::Result;
use kotak_agent::{
    firecracker::{
        client::{FirecrackerClient, SandboxConfig},
        process::FirecrackerProcess,
    },
    network::{IpamAllocator, setup_tap},
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let id = "sandbox-001";
    let ipam = IpamAllocator::new();
    let net = ipam.allocate(id).await?;
    setup_tap(&net).await?;

    let fc_process = FirecrackerProcess::spawn(id).await?;

    let config = SandboxConfig {
        kernel_path: "/home/naufal/kotak/firecracker-local/vmlinux-6.1.155.bin".to_string(),
        rootfs_path: "/home/naufal/kotak/firecracker-local/rootfs.ext4".to_string(),
        tap_name: net.tap_name.clone(),
        mac: "AA:FC:00:00:00:01".to_string(),
        guest_ip: net.guest_ip.clone(),
        gateway_ip: net.host_ip.clone(),
    };

    let fc = FirecrackerClient::new(&fc_process.socket_path);
    fc.launch(&config).await?;

    std::future::pending::<()>().await;

    Ok(())
}
