use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    sync::mpsc,
};

use anyhow::Result;
pub struct VsockClient {
    path: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExecChunk {
    Stdout { data: String },
    Stderr { data: String },
    Exit { code: i32 },
}

impl VsockClient {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    pub async fn exec(&self, command: &str) -> Result<ExecResponse> {
        let mut rx = self.exec_stream(command).await?;
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;

        while let Some(chunk) = rx.recv().await {
            match chunk {
                ExecChunk::Stdout { data } => stdout.push_str(&data),
                ExecChunk::Stderr { data } => stderr.push_str(&data),
                ExecChunk::Exit { code } => exit_code = code,
            }
        }

        Ok(ExecResponse {
            stdout,
            stderr,
            exit_code,
        })
    }

    pub async fn exec_stream(&self, command: &str) -> Result<mpsc::Receiver<ExecChunk>> {
        use tokio::io::BufReader;

        let stream = UnixStream::connect(&self.path).await?;
        let mut stream = BufReader::new(stream);

        stream.get_mut().write_all(b"CONNECT 52\n").await?;
        let mut ack = String::new();
        stream.read_line(&mut ack).await?;
        if !ack.starts_with("OK") {
            anyhow::bail!("vsock failed : {}", ack.trim());
        }

        let request = serde_json::json!({"command": command});
        let request_bytes = serde_json::to_vec(&request)?;
        let len = (request_bytes.len() as u32).to_be_bytes();
        stream.write_all(&len).await?;
        stream.write_all(&request_bytes).await?;

        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            loop {
                let mut len_buf = [0u8; 4];
                if stream.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let len = u32::from_be_bytes(len_buf) as usize;

                let mut buf = vec![0u8; len];
                if stream.read_exact(&mut buf).await.is_err() {
                    break;
                }

                match serde_json::from_slice::<ExecChunk>(&buf) {
                    Ok(chunk) => {
                        let is_exit = matches!(chunk, ExecChunk::Exit { .. });
                        if tx.send(chunk).await.is_err() {
                            break; // receiver dropped
                        }
                        if is_exit {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("chunk parse faill :{}", e);
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}
