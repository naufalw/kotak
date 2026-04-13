use anyhow::Result;
use kotak_guest::handlers::{handle_create_file, handle_exec, handle_mkdir, handle_read_file};
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

    let request: Request = serde_json::from_slice(&buf)?;
    match request {
        Request::Exec { command } => {
            handle_exec(stream, &command).await?;
        }
        Request::Mkdir { path } => handle_mkdir(stream, &path).await?,
        Request::ReadFile { path } => handle_read_file(stream, &path).await?,
        Request::WriteFile { path, content } => handle_create_file(stream, &path, &content).await?,
    }

    Ok(())
}

// #[derive(serde::Deserialize, Debug)]
// struct ExecRequest {
//     command: String,
// }

// #[derive(serde::Serialize)]
// struct ExecResponse {
//     stdout: String,
//     stderr: String,
//     exit_code: i32,
// }

#[derive(serde::Deserialize)]
enum Request {
    Exec { command: String },
    WriteFile { path: String, content: String },
    ReadFile { path: String },
    Mkdir { path: String },
}
