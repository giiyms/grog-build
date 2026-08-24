//! Live unmodified `agy -p` consult. First slice waits for exit and reads stdout.

use std::process::Stdio;

use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::spawn::{ask_agy_argv, provider_turn_argv, AgyMode, AskAntigravitySpec, ProviderTurnSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultResult {
    pub text: String,
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

pub async fn ask_agy(prompt: &str, model: &str) -> Result<ConsultResult, ConsultError> {
    let plan = ask_agy_argv(AskAntigravitySpec {
        prompt,
        model,
        agy_bin: None,
    });
    run_print_plan(&plan.program, &plan.args).await
}

pub async fn provider_turn(prompt: &str, model: &str) -> Result<ConsultResult, ConsultError> {
    let plan = provider_turn_argv(ProviderTurnSpec {
        prompt,
        model,
        mode: AgyMode::AcceptEdits,
        skip_permissions: true,
        extra_add_dir: None,
        resume_conversation: None,
        agy_bin: None,
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
    let mut stdout = String::new();
    if let Some(out) = child.stdout.take() {
        let mut buf = String::new();
        let _ = BufReader::new(out).read_to_string(&mut buf).await;
        stdout = buf;
    }
    let mut stderr = String::new();
    if let Some(err) = child.stderr.take() {
        let mut buf = String::new();
        let _ = BufReader::new(err).read_to_string(&mut buf).await;
        stderr = buf;
    }
    let status = child.wait().await.map_err(|source| ConsultError::Spawn {
        program: program.to_string(),
        source,
    })?;
    if !status.success() {
        return Err(ConsultError::Exit {
            program: program.to_string(),
            status: status.code().unwrap_or(1),
            stderr: stderr.trim().to_string(),
        });
    }
    let text = stdout.trim().to_string();
    if text.is_empty() {
        return Err(ConsultError::Empty {
            program: program.to_string(),
        });
    }
    Ok(ConsultResult { text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[tokio::test]
    async fn consult_reads_agy_stdout() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("agy");
        fs::write(&bin, "#!/bin/sh\necho gemini-ok\n").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        let out = run_print_plan(bin.to_str().unwrap(), &["-p".into(), "hi".into()])
            .await
            .unwrap();
        assert_eq!(out.text, "gemini-ok");
    }
}
