use anyhow::Result;

pub async fn run_cmd(args: &[&str]) -> Result<()> {
    let status = tokio::process::Command::new(args[0])
        .args(&args[1..])
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "command failed: `{}` exit code {:?}",
            args.join(" "),
            status.code()
        ));
    }

    Ok(())
}
