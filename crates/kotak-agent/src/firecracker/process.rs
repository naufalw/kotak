use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;

pub struct FirecrackerProcess {
    child: tokio::process::Child,
    pub socket_path: String,
    pub vsock_path: String,
}

impl FirecrackerProcess {
    pub async fn spawn(id: &str) -> Result<Self> {
        let socket_path = format!("/tmp/firecracker-{}.sock", id);
        let vsock_path = format!("/tmp/firecracker-{}-vsock.sock", id);

        // Clean up stale sockets
        let _ = tokio::fs::remove_file(&socket_path).await;
        let _ = tokio::fs::remove_file(&vsock_path).await;

        let child = tokio::process::Command::new("firecracker")
            .args(["--api-sock", &socket_path, "--id", id])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        for _ in 0..20 {
            if tokio::fs::metadata(&socket_path).await.is_ok() {
                return Ok(Self {
                    child,
                    socket_path,
                    vsock_path,
                });
            }
            sleep(Duration::from_millis(50)).await;
        }

        Err(anyhow::anyhow!(
            "firecracker socket never appeared at {}",
            socket_path
        ))
    }
}

impl Drop for FirecrackerProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(&self.vsock_path);
    }
}
