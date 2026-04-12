use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post},
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    filesystem::FilesystemManager,
    network::{IpamAllocator, PortManager},
    sandbox::{Sandbox, SandboxConfig, resume},
    snapshot::SnapshotStore,
};

pub struct AppState {
    pub sandboxes: Mutex<HashMap<String, Sandbox>>,
    pub ipam: IpamAllocator,
    pub port_manager: PortManager,
    pub store: SnapshotStore,
    pub config: SandboxConfig,
    pub base_rootfs: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sandboxes", get(list_sandboxes))
        .route("/sandboxes/create", post(create_sandbox))
        .route("/sandboxes/{id}", delete(delete_sandbox))
        .route("/sandboxes/{id}/exec", post(exec_sandbox))
        .route("/sandboxes/{id}/hibernate", post(hibernate_sandbox))
        .route("/sandboxes/{id}/ports/{port}", post(forward_port))
        .route("/sandboxes/{id}/ports/{port}", delete(remove_port))
        .route("/sandboxes/{id}/resume", post(resume_sandbox))
        .with_state(state)
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

#[derive(Serialize)]
struct PortForwardResponse {
    guest_port: u16,
    host_port: u16,
}

// handlers
//
async fn list_sandboxes(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sandboxes = state.sandboxes.lock().await;
    let list: Vec<SandboxResponse> = sandboxes
        .values()
        .map(|s| SandboxResponse {
            id: s.id.clone(),
            guest_ip: s.net.guest_ip.clone(),
        })
        .collect();
    Json(list).into_response()
}

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

async fn delete_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let sandbox = state.sandboxes.lock().await.remove(&id);
    match sandbox {
        Some(s) => match s.destroy(&state.ipam, &state.port_manager).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn exec_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ExecRequest>,
) -> impl IntoResponse {
    let sandboxes = state.sandboxes.lock().await;
    match sandboxes.get(&id) {
        Some(sbx) => match sbx.exec(&body.command).await {
            Ok(r) => Json(ExecResponse {
                stdout: r.stdout,
                stderr: r.stderr,
                exit_code: r.exit_code,
            })
            .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn hibernate_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut sandboxes = state.sandboxes.lock().await;
    match sandboxes.get(&id) {
        Some(sandbox) => match sandbox.hibernate(&state.store).await {
            Ok(_) => {
                sandboxes.remove(&id);
                StatusCode::NO_CONTENT.into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn resume_sandbox(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.sandboxes.lock().await.contains_key(&id) {
        return (StatusCode::CONFLICT, "sandbox already running").into_response();
    }

    let fs = FilesystemManager::new(&state.base_rootfs);
    match resume(&id, &state.ipam, fs, &state.store, &state.config).await {
        Ok(sbx) => {
            let guest_ip = sbx.net.guest_ip.clone();
            state.sandboxes.lock().await.insert(id.clone(), sbx);
            Json(SandboxResponse { id, guest_ip }).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn forward_port(
    State(state): State<Arc<AppState>>,
    Path((id, guest_port)): Path<(String, u16)>,
) -> impl IntoResponse {
    let sandboxes = state.sandboxes.lock().await;
    match sandboxes.get(&id) {
        Some(sandbox) => match sandbox.forward_port(&state.port_manager, guest_port).await {
            Ok(host_port) => Json(PortForwardResponse {
                guest_port,
                host_port,
            })
            .into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn remove_port(
    State(state): State<Arc<AppState>>,
    Path((id, guest_port)): Path<(String, u16)>,
) -> impl IntoResponse {
    let sandboxes = state.sandboxes.lock().await;
    match sandboxes.get(&id) {
        Some(sandbox) => match sandbox.remove_port(&state.port_manager, guest_port).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
