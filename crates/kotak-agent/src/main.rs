use anyhow::Result;
use kotak_agent::{
    firecracker::client::{FirecrackerClient, SandboxConfig},
    network::{IpamAllocator, setup_tap},
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let ipam = IpamAllocator::new();

    let net = ipam.allocate("sandbox-001").await?;
    setup_tap(&net).await?;
    let config = SandboxConfig {
        kernel_path: "./vmlinux-6.1.155.bin".to_string(),
        rootfs_path: "./rootfs.ext4".to_string(),
        tap_name: net.tap_name.clone(),
        mac: "AA:FC:00:00:00:01".to_string(),
        guest_ip: net.guest_ip.clone(),
        gateway_ip: net.host_ip.clone(),
    };

    let fc = FirecrackerClient::new("/tmp/firecracker.socket");
    fc.launch(&config).await?;

    Ok(())
}
