use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&request.command)
        .output()
        .await?;

    let response = ExecResponse {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    };

    let response_bytes = serde_json::to_vec(&response)?;
    let len = (response_bytes.len() as u32).to_be_bytes();

    stream.write_all(&len).await?;
    stream.write_all(&response_bytes).await?;

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
