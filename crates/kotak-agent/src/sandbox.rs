use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    filesystem::FilesystemManager,
    firecracker::{
        client::{FirecrackerClient, ResolvedConfig},
        process::FirecrackerProcess,
    },
    network::{IpamAllocator, PortForward, PortManager, TapNetwork, setup_tap, teardown_tap},
    snapshot::SnapshotStore,
    vsock::{ExecChunk, ExecResponse, VsockClient},
};
use anyhow::Result;
use tokio::sync::{Mutex, mpsc};

pub struct SandboxConfig {
    pub kernel_path: String,
    pub guest_cid: u32,
}

pub struct Sandbox {
    pub id: String,
    pub net: TapNetwork,
    pub port_forwards: Mutex<Vec<PortForward>>,
    pub last_active: Arc<AtomicU64>,
    process: FirecrackerProcess,
    client: FirecrackerClient,
    vsock: VsockClient,
    fs: FilesystemManager,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// vm boot : TAP, firecracker, configure, start
async fn boot_vm(
    id: &str,
    rootfs_path: &Path,
    net: &TapNetwork,
    config: &SandboxConfig,
) -> Result<(FirecrackerProcess, FirecrackerClient, VsockClient)> {
    let mac = format!("AA:FC:00:00:{:02X}:{:02X}", net.slot >> 8, net.slot & 0xff);
    setup_tap(net).await?;

    let process = FirecrackerProcess::spawn(id).await?;
    let client = FirecrackerClient::new(&process.socket_path);
    let vsock = VsockClient::new(&process.vsock_path);

    let resolved = ResolvedConfig {
        kernel_path: &config.kernel_path,
        rootfs_path: rootfs_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 rootfs path"))?,
        mac: &mac,
        guest_cid: config.guest_cid,
        tap_name: &net.tap_name,
        guest_ip: &net.guest_ip,
        gateway_ip: &net.host_ip,
        vsock_path: &process.vsock_path,
    };

    client.launch(&resolved).await?;
    Ok((process, client, vsock))
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
        let (process, client, vsock) = boot_vm(id, &rootfs_path, &net, config).await?;

        vsock
            .exec("rm -f /etc/ssh/ssh_host_* && ssh-keygen -A && rc-service sshd restart")
            .await?;

        Ok(Self {
            id: id.to_string(),
            process,
            client,
            net,
            fs,
            vsock,
            last_active: Arc::new(AtomicU64::new(now_secs())),
            port_forwards: Mutex::new(Vec::new()),
        })
    }

    pub async fn resume(
        id: &str,
        ipam: &IpamAllocator,
        fs: FilesystemManager,
        store: &SnapshotStore,
        config: &SandboxConfig,
    ) -> Result<Self> {
        let rootfs_path = fs.prepare_empty(id).await?;
        store.restore_filesystem(id, &rootfs_path).await?;

        let net = ipam.allocate(id).await?;
        let (process, client, vsock) = boot_vm(id, &rootfs_path, &net, config).await?;

        Ok(Self {
            id: id.to_string(),
            process,
            client,
            net,
            fs,
            vsock,
            last_active: Arc::new(AtomicU64::new(now_secs())),
            port_forwards: Mutex::new(Vec::new()),
        })
    }

    pub async fn exec(&self, command: &str) -> Result<ExecResponse> {
        self.touch();
        self.vsock.exec(command).await
    }

    pub async fn exec_stream(&self, command: &str) -> Result<mpsc::Receiver<ExecChunk>> {
        self.touch();
        self.vsock.exec_stream(command).await
    }

    pub async fn hibernate(
        self,
        store: &SnapshotStore,
        ipam: &IpamAllocator,
        port_manager: &PortManager,
    ) -> Result<()> {
        self.client.stop().await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        store
            .snapshot_filesystem(&self.id, &self.fs.rootfs_path(&self.id))
            .await?;
        self.destroy(ipam, port_manager).await
    }

    pub fn last_active_secs(&self) -> u64 {
        self.last_active.load(Ordering::Relaxed)
    }

    pub async fn forward_port(&self, port_manager: &PortManager, guest_port: u16) -> Result<u16> {
        let host_port = port_manager
            .forward(&self.id, &self.net.guest_ip, guest_port)
            .await?;
        self.port_forwards.lock().await.push(PortForward {
            host_port,
            guest_port,
        });

        Ok(host_port)
    }

    pub async fn remove_port(&self, port_manager: &PortManager, guest_port: u16) -> Result<()> {
        let mut forwards = self.port_forwards.lock().await;
        if let Some(pos) = forwards.iter().position(|f| f.guest_port == guest_port) {
            let fwd = forwards.remove(pos);
            port_manager
                .remove(fwd.host_port, &self.net.guest_ip, guest_port)
                .await?;
        }
        Ok(())
    }

    pub fn touch(&self) {
        self.last_active.store(now_secs(), Ordering::Relaxed);
    }

    pub async fn destroy(self, ipam: &IpamAllocator, port_manager: &PortManager) -> Result<()> {
        if let Err(e) = self.client.stop().await {
            tracing::warn!("sandbox {}: stop failed: {}", self.id, e);
        }
        for fwd in self.port_forwards.lock().await.iter() {
            if let Err(e) = port_manager
                .remove(fwd.host_port, &self.net.guest_ip, fwd.guest_port)
                .await
            {
                tracing::warn!("sandbox {}: port remove failed: {}", self.id, e);
            }
        }
        teardown_tap(&self.net).await?;
        ipam.release(self.net.slot).await;
        self.fs.teardown(&self.id).await?;
        Ok(())
    }
}
