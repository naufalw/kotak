use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use tokio_vsock::{VMADDR_CID_ANY, VsockAddr, VsockListener};

const AGENT_PORT: u32 = 52;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("vsock portt {}", AGENT_PORT);

    let listener = VsockListener::bind(VsockAddr::new(VMADDR_CID_ANY, AGENT_PORT))?;

    tracing::info!("READYYY");

    loop {
        let (mut stream, addr) = listener.accept().await?;
        tracing::info!("connection from {:?}", addr);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(&mut stream).await {
                tracing::error!("connection error: {}", e);
            }
        });
    }
}

async fn handle_connection<S>(stream: &mut S) -> Result<()>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    let request: ExecRequest = serde_json::from_slice(&buf)?;
    tracing::info!("exec: {:?}", request.command);

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&request.command)
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

async fn send_chunk<S>(stream: &mut S, value: serde_json::Value) -> Result<()>
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

#[derive(serde::Deserialize, Debug)]
struct ExecRequest {
    command: String,
}

#[derive(serde::Serialize)]
struct ExecResponse {
    stdout: String,
    stderr: String,
    exit_code: i32,
}
