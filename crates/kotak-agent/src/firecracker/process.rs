use std::time::Duration;

use anyhow::Result;
use tokio::time::sleep;

pub struct FirecrackerProcess {
    child: tokio::process::Child,
    pub socket_path: String,
}

impl FirecrackerProcess {
    pub async fn spawn(id: &str) -> Result<Self> {
        let socket_path = format!("/tmp/firecracker-{}.sock", id);

        let _ = tokio::fs::remove_file(&socket_path).await;

        let child = tokio::process::Command::new("firecracker")
            .args(["--api-sock", &socket_path, "--id", id])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        for _ in 0..20 {
            if tokio::fs::metadata(&socket_path).await.is_ok() {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        Ok(Self { child, socket_path })
    }
}
