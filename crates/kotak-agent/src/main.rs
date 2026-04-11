use anyhow::Result;
use kotak_agent::firecracker::{FirecrackerClient, SandboxConfig};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = SandboxConfig {
        kernel_path: "./vmlinux-6.1.155.bin".to_string(),
        rootfs_path: "./rootfs.ext4".to_string(),
        tap_name: "tap0".to_string(),
        mac: "AA:FC:00:00:00:01".to_string(),
        guest_ip: "172.16.0.2".to_string(),
        gateway_ip: "172.16.0.1".to_string(),
    };

    let fc = FirecrackerClient::new("/tmp/firecracker.socket");
    fc.launch(&config).await?;

    Ok(())
}
