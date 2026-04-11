use std::path::Path;

use anyhow::Result;
use aws_config::Region;
use aws_credential_types::Credentials;
use aws_sdk_s3::{Client, primitives::ByteStream};

const BUCKET: &str = "kotaksnapshot";

pub struct SnapshotStore {
    client: Client,
}

impl SnapshotStore {
    pub fn new() -> Self {
        let creds = Credentials::new(
            "IiB9vb311mxJIDqrDsrb",
            "ga0mtWIY6cQS3HqRO3teimYjahO69qsqcm8vpUHv",
            None,
            None,
            "rustfs",
        );

        let config = aws_sdk_s3::Config::builder()
            .credentials_provider(creds)
            .region(Region::new("ap-southeast-3"))
            .endpoint_url("http://localhost:9000")
            .force_path_style(true)
            .build();

        Self {
            client: Client::from_conf(config),
        }
    }

    pub async fn upload(
        &self,
        sandbox_id: &str,
        local_path: &Path,
        key_suffix: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", sandbox_id, key_suffix);
        let body = ByteStream::from_path(local_path).await?;

        self.client
            .put_object()
            .bucket(BUCKET)
            .key(&key)
            .body(body)
            .send()
            .await?;

        tracing::info!(
            "uploaded {} to s3://{}/{}",
            local_path.display(),
            BUCKET,
            key
        );
        Ok(())
    }

    pub async fn download(&self, sandbox_id: &str, key_suffix: &str, dest: &Path) -> Result<()> {
        let key = format!("{}/{}", sandbox_id, key_suffix);

        let resp = self
            .client
            .get_object()
            .bucket(BUCKET)
            .key(&key)
            .send()
            .await?;

        let bytes = resp.body.collect().await?.into_bytes();
        tokio::fs::write(dest, bytes).await?;

        tracing::info!("downloaded s3://{}/{} to {}", BUCKET, key, dest.display());
        Ok(())
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new()
    }
}
