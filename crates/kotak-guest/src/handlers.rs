use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::util::send_chunk;

pub async fn handle_exec<S>(stream: &mut S, command: &str) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    tracing::info!("exec: {:?}", command);

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();

    loop {
        tokio::select! {
            line = stdout.next_line() => {
                match line? {
                    Some(l) => send_chunk(stream, serde_json::json!({"type": "stdout", "data": l + "\n"})).await?,
                    None => break,
                }
            }
            line = stderr.next_line() => {
                if let Some(l) = line? { send_chunk(stream, serde_json::json!({"type": "stderr", "data": l + "\n"})).await? }
            }
        }
    }

    let status = child.wait().await?;
    let code = status.code().unwrap_or(-1);
    send_chunk(stream, serde_json::json!({"type": "exit", "code": code})).await?;

    Ok(())
}

pub async fn handle_mkdir<S>(stream: &mut S, path: &str) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    match tokio::fs::create_dir_all(&path).await {
        Ok(_) => send_chunk(stream, serde_json::json!({"type": "exit", "code": 0})).await,
        Err(e) => {
            send_chunk(
                stream,
                serde_json::json!({"type": "error", "message": e.to_string()}),
            )
            .await
        }
    }
}

pub async fn handle_read_file<S>(stream: &mut S, path: &str) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    use base64::Engine;
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            send_chunk(
                stream,
                serde_json::json!({"type": "file", "content": encoded}),
            )
            .await?;
        }
        Err(e) => {
            send_chunk(
                stream,
                serde_json::json!({"type": "error", "message": e.to_string()}),
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn handle_create_file<S>(stream: &mut S, path: &str, content: &str) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    use base64::Engine;

    let bytes = base64::engine::general_purpose::STANDARD.decode(content)?;

    if let Some(parent) = std::path::Path::new(path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, bytes).await?;

    send_chunk(stream, serde_json::json!({"type": "exit", "code": 0})).await
}
