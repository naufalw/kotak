use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use kotak_agent::{
    api::{AppState, router},
    network::{IpamAllocator, PortManager},
    sandbox::SandboxConfig,
    snapshot::SnapshotStore,
};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let state = Arc::new(AppState {
        sandboxes: Mutex::new(HashMap::new()),
        ipam: IpamAllocator::new(),
        store: SnapshotStore::new(),
        port_manager: PortManager::new(),
        config: SandboxConfig {
            kernel_path: "/home/naufal/kotak/firecracker-local/vmlinux-6.1.155.bin".to_string(),
            guest_cid: 3,
        },
        base_rootfs: "/home/naufal/kotak/firecracker-local/rootfs.ext4".to_string(),
    });

    let app = router(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("here AT 0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
