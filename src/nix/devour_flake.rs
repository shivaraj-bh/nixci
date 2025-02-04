//! Rust support for invoking <https://github.com/srid/devour-flake>

use anyhow::{bail, Context, Result};
use std::{collections::HashSet, process::Stdio, str::FromStr};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Absolute path to the devour-flake executable
///
/// We expect this environment to be set in Nix build and shell.
pub const DEVOUR_FLAKE: &str = env!("DEVOUR_FLAKE");

pub struct DevourFlakeOutput(pub HashSet<DrvOut>);

impl FromStr for DevourFlakeOutput {
    type Err = anyhow::Error;

    fn from_str(output_filename: &str) -> Result<Self, Self::Err> {
        // Read output_filename, as newline separated strings
        let raw_output = std::fs::read_to_string(output_filename)?;
        let outs = raw_output.split_ascii_whitespace();
        let outs: HashSet<DrvOut> = outs.map(|s| DrvOut(s.to_string())).collect();
        if outs.is_empty() {
            bail!(
                "devour-flake produced an outpath ({}) with no outputs",
                output_filename
            );
        } else {
            Ok(DevourFlakeOutput(outs))
        }
    }
}

/// Nix derivation output path
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Clone, Hash)]
pub struct DrvOut(pub String);

pub async fn devour_flake(verbose: bool, args: Vec<String>) -> Result<DevourFlakeOutput> {
    // TODO: Use nix_rs here as well
    // In the context of doing https://github.com/srid/nixci/issues/15
    let nix = crate::nixcmd().await;
    let mut cmd = nix.command();
    let devour_flake_url = format!("{}#default", env!("DEVOUR_FLAKE"));
    cmd.args([
        "build",
        &devour_flake_url,
        "-L",
        "--no-link",
        "--print-out-paths",
        "--override-input",
        "flake",
    ])
    .args(args);
    nix_rs::command::trace_cmd(&cmd);
    let mut output_fut = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let stderr_handle = output_fut.stderr.take().unwrap();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr_handle).lines();
        while let Some(line) = reader.next_line().await.expect("read stderr") {
            if !verbose {
                if line.starts_with("• Added input") {
                    // Consume the input logging itself
                    reader.next_line().await.expect("read stderr");
                    continue;
                } else if line.starts_with("warning: not writing modified lock file of flake") {
                    continue;
                }
            }
            eprintln!("{}", line);
        }
    });
    let output = output_fut
        .wait_with_output()
        .await
        .context("Unable to spawn devour-flake process")?;
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let v = DevourFlakeOutput::from_str(stdout.trim())?;
        Ok(v)
    } else {
        let exit_code = output.status.code().unwrap_or(1);
        bail!("devour-flake failed to run (exited: {})", exit_code);
    }
}
