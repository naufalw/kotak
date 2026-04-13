use anyhow::Result;
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

pub async fn handle_exec(stream: &mut (impl AsyncWriteExt + Unpin), command: &str) -> Result<()> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout pipe missing despite Stdio::piped()");
    let stderr = child.stderr.take().expect("stderr pipe missing despite Stdio::piped()");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line? {
                    Some(l) => send_chunk(stream, json!({"type": "stdout", "data": l + "\n"})).await?,
                    None => break,
                }
            }
            line = stderr_reader.next_line() => {
                if let Some(l) = line? { send_chunk(stream, json!({"type": "stderr", "data": l + "\n"})).await? }
            }
        }
    }

    let status = child.wait().await?;
    let code = status.code().unwrap_or(-1);
    send_chunk(stream, json!({"type": "exit", "code": code})).await?;

    Ok(())
}

async fn send_chunk(
    stream: &mut (impl AsyncWriteExt + Unpin),
    value: serde_json::Value,
) -> Result<()> {
    let bytes = serde_json::to_vec(&value)?;
    let len = (bytes.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}
