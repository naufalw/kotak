use anyhow::Result;
use http_body_util::Full;
use hyper::{Method, Request, body::Bytes};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::net::UnixStream;

pub struct FirecrackerClient {
    socket_path: String,
}

impl FirecrackerClient {
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
        }
    }

    async fn put(&self, path: &str, body: serde_json::Value) -> Result<()> {
        let stream = UnixStream::connect(&self.socket_path).await?;
        let io = TokioIo::new(stream);

        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
        tokio::spawn(conn);

        let body_str = serde_json::to_string(&body).unwrap();
        let req = Request::builder()
            .method(Method::PUT)
            .uri(format!("http://localhost{}", path))
            .header("Content-Type", "application/json")
            .body(Full::new(Bytes::from(body_str)))?;

        let res = sender.send_request(req).await?;
        tracing::debug!("PUT {} -> {}", path, res.status());
        if !res.status().is_success() {
            return Err(anyhow::anyhow!("Firecracker API error: {}", res.status()));
        }

        Ok(())
    }

    pub async fn configure_boot(&self, kernel_path: &str, boot_args: &str) -> Result<()> {
        self.put(
            "/boot-source",
            json!({
                "kernel_image_path": kernel_path,
                "boot_args": boot_args
            }),
        )
        .await
    }

    pub async fn configure_device(&self, rootfs_path: &str) -> Result<()> {
        self.put(
            "/drives/rootfs",
            json!({
                "drive_id": "rootfs",
                            "path_on_host": rootfs_path,
                            "is_root_device": true,
                            "is_read_only": false
            }),
        )
        .await
    }

    pub async fn configure_network(&self, tap_name: &str, mac: &str) -> Result<()> {
        self.put(
            "/network-interface/eth0",
            json!({
                "iface_id": "eth0",
                "guest_mac": mac,
                "host_dev_name": tap_name

            }),
        )
        .await
    }

    pub async fn start(&self) -> Result<()> {
        self.put(
            "/actions",
            json!({
                "action_type": "InstanceStart"
            }),
        )
        .await
    }
}
