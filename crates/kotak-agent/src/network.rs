use std::collections::HashMap;

use anyhow::{Result, anyhow};
use tokio::sync::Mutex;

// Every sandbox gets /30 => 2 usable
// 172.16.{slot}.0/30
//   host (gateway): 172.16.{slot}.1
//   guest:          172.16.{slot}.2
pub struct IpamAllocator {
    // slot -> sandbox_id
    leases: Mutex<HashMap<u32, String>>,
}

impl IpamAllocator {
    pub fn new() -> Self {
        Self {
            leases: Mutex::new(HashMap::new()),
        }
    }

    pub async fn allocate(&self, sandbox_id: &str) -> Result<TapNetwork> {
        let mut leases = self.leases.lock().await;

        let slot = (1u32..=254)
            .find(|s| !leases.contains_key(s))
            .ok_or_else(|| anyhow!("No free IP from IP pool"))?;

        leases.insert(slot, sandbox_id.to_string());

        Ok(TapNetwork {
            tap_name: format!("tap-{}", slot),
            host_ip: format!("172.16.{}.1", slot),
            guest_ip: format!("172.16.{}.2", slot),
            cidr: format!("172.16.{}.1/30", slot),
            slot,
        })
    }

    pub async fn release(&self, slot: u32) {
        self.leases.lock().await.remove(&slot);
    }
}

impl Default for IpamAllocator {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TapNetwork {
    pub tap_name: String,
    pub host_ip: String,  // 172.16.{slot}.1
    pub guest_ip: String, // 172.16.{slot}.2
    pub cidr: String,     // 172.16.{slot}.1/30 — assigned to TAP
    pub slot: u32,
}

pub async fn setup_tap(net: &TapNetwork) -> Result<()> {
    tokio::process::Command::new("ip")
        .args(["tuntap", "add", &net.tap_name, "mode", "tap"])
        .status()
        .await?;

    tokio::process::Command::new("ip")
        .args(["addr", "add", &net.cidr, "dev", &net.tap_name])
        .status()
        .await?;

    tokio::process::Command::new("ip")
        .args(["link", "set", &net.tap_name, "up"])
        .status()
        .await?;

    Ok(())
}

pub async fn teardown_tap(net: &TapNetwork) -> Result<()> {
    tokio::process::Command::new("ip")
        .args(["link", "del", &net.tap_name])
        .status()
        .await?;
    Ok(())
}
