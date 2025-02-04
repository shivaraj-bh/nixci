use std::process::Stdio;

use anyhow::{bail, Result};
use nix_rs::flake::url::FlakeUrl;

/// Make sure that the `flake.lock` file is in sync.
pub async fn nix_flake_lock_check(url: &FlakeUrl) -> Result<()> {
    let nix = crate::nixcmd().await;
    let mut cmd = nix.command();
    cmd.args(["flake", "lock", "--no-update-lock-file", &url.0]);
    nix_rs::command::trace_cmd(&cmd);
    let status = cmd.stdin(Stdio::null()).spawn()?.wait().await?;
    if status.success() {
        Ok(())
    } else {
        let exit_code = status.code().unwrap_or(1);
        bail!("nix flake lock failed to run (exited: {})", exit_code);
    }
}
