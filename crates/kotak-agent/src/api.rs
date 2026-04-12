use std::{collections::HashMap, sync::Arc};

use axum::{Json, Router, extract::State, response::IntoResponse};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    filesystem::FilesystemManager,
    network::IpamAllocator,
    sandbox::{Sandbox, SandboxConfig},
    snapshot::SnapshotStore,
};

pub struct AppState {
    pub sandboxes: Mutex<HashMap<String, Sandbox>>,
    pub ipam: IpamAllocator,
    pub store: SnapshotStore,
    pub config: SandboxConfig,
    pub base_rootfs: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new().with_state(state)
}

// Packet Structure

#[derive(Serialize)]
struct SandboxResponse {
    id: String,
    guest_ip: String,
}

#[derive(Deserialize)]
struct ExecRequest {
    command: String,
}

#[derive(Serialize)]
struct ExecResponse {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

// handlers
//

async fn create_sandbox(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let id = uuid::Uuid::new_v4().to_string();
    let fs = FilesystemManager::new(&state.base_rootfs);

    match Sandbox::create(&id, &state.ipam, fs, &state.config).await {
        Ok(sbx) => {
            let guest_ip = sbx.net.guest_ip.clone();
            state.sandboxes.lock().await.insert(id.clone(), sbx);
            (StatusCode::CREATED, Json(SandboxResponse { id, guest_ip })).into_response()
        }
        Err(e) => {
            tracing::error!("create fail: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

