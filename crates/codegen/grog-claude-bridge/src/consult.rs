//! Live `claude` print-mode consult. The child uses `~/.claude` credentials.

use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::models::{AskClaudeMode, LongContextSettings};
use crate::spawn::{ask_claude_argv, provider_turn_argv, AskClaudeSpec, ProviderTurnSpec};
use crate::stream::{parse_stream_json_line, StreamEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultResult {
    pub text: String,
    pub session_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConsultError {
    #[error("failed to spawn `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`{program}` exited {status}: {stderr}")]
    Exit {
        program: String,
        status: i32,
        stderr: String,
    },
    #[error("`{program}` produced no assistant text")]
    Empty { program: String },
}

/// Isolated AskClaude consult (`read` tools, no session persistence).
pub async fn ask_claude(prompt: &str, model_id: &str) -> Result<ConsultResult, ConsultError> {
    let plan = ask_claude_argv(AskClaudeSpec {
        prompt,
        model_id,
        settings: LongContextSettings::default(),
        mode: AskClaudeMode::Read,
        isolated: true,
        claude_bin: None,
    });
    run_print_plan(&plan.program, &plan.args).await
}

/// Full-session provider turn (no loopback MCP yet).
pub async fn provider_turn(prompt: &str, model_id: &str) -> Result<ConsultResult, ConsultError> {
    let plan = provider_turn_argv(ProviderTurnSpec {
        prompt,
        model_id,
        settings: LongContextSettings::default(),
        resume_session: None,
        mcp_config_path: None,
        claude_bin: None,
    });
    run_print_plan(&plan.program, &plan.args).await
}

pub async fn run_print_plan(program: &str, args: &[String]) -> Result<ConsultResult, ConsultError> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|source| ConsultError::Spawn {
        program: program.to_string(),
        source,
    })?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let mut lines = BufReader::new(stdout).lines();
    let mut text = String::new();
    let mut session_id = None;
    while let Ok(Some(line)) = lines.next_line().await {
        match parse_stream_json_line(&line) {
            Ok(StreamEvent::TextDelta(delta)) => text.push_str(&delta),
            Ok(StreamEvent::Result {
                session_id: sid,
                text: result,
            }) => {
                session_id = sid;
                if text.is_empty() && !result.is_empty() {
                    text = result;
                }
            }
            Ok(StreamEvent::Error(err)) => {
                let _ = child.kill().await;
                return Err(ConsultError::Exit {
                    program: program.to_string(),
                    status: 1,
                    stderr: err,
                });
            }
            _ => {}
        }
    }
    let mut err_buf = String::new();
    let _ = BufReader::new(stderr).read_line(&mut err_buf).await;
    let status = child.wait().await.map_err(|source| ConsultError::Spawn {
        program: program.to_string(),
        source,
    })?;
    if !status.success() {
        return Err(ConsultError::Exit {
            program: program.to_string(),
            status: status.code().unwrap_or(1),
            stderr: err_buf.trim().to_string(),
        });
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(ConsultError::Empty {
            program: program.to_string(),
        });
    }
    Ok(ConsultResult { text, session_id })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[tokio::test]
    async fn consult_parses_stream_json_from_fake_claude() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("claude");
        fs::write(
            &bin,
            r#"#!/bin/sh
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"pong"}]}}'
echo '{"type":"result","session_id":"s1","result":"pong"}'
"#,
        )
        .unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let out = run_print_plan(bin.to_str().unwrap(), &["-p".into(), "ping".into()])
            .await
            .unwrap();
        assert_eq!(out.text, "pong");
        assert_eq!(out.session_id.as_deref(), Some("s1"));
    }
}
