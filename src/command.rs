use std::{process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};

use crate::{
    approval,
    config::Config,
    error::{Result, ToolError},
    text::truncate,
    workspace::Workspace,
};

/// PowerShell on Windows, sh elsewhere. Runs in the server's working directory
/// unless `cwd` is given. Asks the human first unless FS_MCP_AUTO_APPROVE_COMMANDS=1.
pub async fn run_command(
    ws: &Workspace,
    cfg: &Config,
    command: &str,
    cwd: Option<&str>,
    timeout_secs: Option<u64>,
) -> Result<String> {
    let dir = match cwd {
        Some(c) => ws.resolve(c)?,
        None => ws.base(),
    };
    if !dir.is_dir() {
        return Err(ToolError::msg(format!("'{}' is not a directory", dir.display())));
    }

    if cfg.require_command_approval {
        let q = format!("Run in '{}':\n    {command}", dir.display());
        if !approval::confirm(&q).await {
            return Err(ToolError::msg("The user denied the command. Nothing was run."));
        }
    }

    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    cmd.current_dir(&dir)
        .stdin(Stdio::null()) // never steal the approval prompt's stdin
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let secs = timeout_secs.unwrap_or(60).clamp(1, 600);
    let child = cmd.spawn()?;
    let output = match timeout(Duration::from_secs(secs), child.wait_with_output()).await {
        Err(_) => return Err(ToolError::msg(format!("command timed out after {secs}s and was killed"))),
        Ok(res) => res?,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().map_or("signal".to_string(), |c| c.to_string());
    Ok(format!(
        "exit code: {code}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        truncate(&stdout, cfg.max_output_chars),
        truncate(&stderr, cfg.max_output_chars),
    ))
}
