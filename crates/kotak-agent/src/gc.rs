use tracing::info;

use crate::api::AppState;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn start_gc(state: Arc<AppState>, idle_secs: u64) {
    info!("garbage collector starts, timeout={}", idle_secs);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let idle: Vec<String> = {
                let sandboxes = state.sandboxes.read().await;
                sandboxes
                    .values()
                    .filter(|s| now.saturating_sub(s.last_active_secs()) > idle_secs)
                    .map(|s| s.id.clone())
                    .collect()
            };

            for id in idle {
                tracing::info!("gc: hibernating idle sandbox {}", id);
                let sandbox = state.sandboxes.write().await.remove(&id);
                let Some(s) = sandbox else { continue };
                if let Err(e) = s
                    .hibernate(&state.store, &state.ipam, &state.port_manager)
                    .await
                {
                    tracing::error!("gc: hibernate failed for {}: {}", id, e);
                }
            }
        }
    });
}
