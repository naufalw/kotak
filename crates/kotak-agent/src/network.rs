use anyhow::Result;

pub struct TapConfig {
    pub tap_name: String,
    pub host_ip: String,
}

pub async fn setup_tap(config: &TapConfig) -> Result<()> {
    tokio::process::Command::new("ip")
        .args(["tuntap", "add", &config.tap_name, "mode", "tap"])
        .status()
        .await?;

    tokio::process::Command::new("ip")
        .args(["addr", "add", &config.host_ip, "dev", &config.tap_name])
        .status()
        .await?;

    tokio::process::Command::new("ip")
        .args(["link", "set", &config.tap_name, "up"])
        .status()
        .await?;

    Ok(())
}

pub async fn teardown_tap(tap_name: &str) -> Result<()> {
    tokio::process::Command::new("ip")
        .args(["link", "del", tap_name])
        .status()
        .await?;
    Ok(())
}
