use anyhow::Result;
use tokio::io::AsyncWriteExt;

pub async fn send_chunk<S>(stream: &mut S, value: serde_json::Value) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    let bytes = serde_json::to_vec(&value)?;
    let len = (bytes.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&bytes).await?;
    stream.flush().await?;
    Ok(())
}
