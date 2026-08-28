//! Argv for unmodified `agy`. Default skip-permissions so `-p` cannot hang
//! on an unanswerable y/n prompt (upstream antigravity-cli#318).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgySpawnPlan {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgyMode {
    AcceptEdits,
    Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTurnSpec<'a> {
    pub prompt: &'a str,
    pub model: &'a str,
    pub mode: AgyMode,
    pub skip_permissions: bool,
    pub extra_add_dir: Option<&'a str>,
    pub resume_conversation: Option<&'a str>,
    pub agy_bin: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskAntigravitySpec<'a> {
    pub prompt: &'a str,
    pub model: &'a str,
    pub agy_bin: Option<&'a str>,
}

pub fn provider_turn_argv(spec: ProviderTurnSpec<'_>) -> AgySpawnPlan {
    let mut args = vec!["-p".into()];
    args.extend(["--model".into(), spec.model.into()]);
    match spec.mode {
        AgyMode::Plan => args.extend(["--mode".into(), "plan".into()]),
        AgyMode::AcceptEdits => args.extend(["--mode".into(), "accept-edits".into()]),
    }
    if spec.skip_permissions {
        args.push("--dangerously-skip-permissions".into());
    }
    if let Some(dir) = spec.extra_add_dir {
        args.extend(["--add-dir".into(), dir.into()]);
    }
    if let Some(id) = spec.resume_conversation {
        args.extend(["--conversation".into(), id.into()]);
    }
    args.push(spec.prompt.into());
    AgySpawnPlan {
        program: spec.agy_bin.unwrap_or("agy").to_string(),
        args,
    }
}

/// AskAntigravity: Plan mode (no writes) and no grog-tool MCP dir.
/// `--dangerously-skip-permissions` stays so `-p` cannot hang on y/n
/// (upstream antigravity-cli#318). Plan mode is the no-write escape;
/// AcceptEdits is only for full `/model antigravity/…` turns.
pub fn ask_agy_argv(spec: AskAntigravitySpec<'_>) -> AgySpawnPlan {
    provider_turn_argv(ProviderTurnSpec {
        prompt: spec.prompt,
        model: spec.model,
        mode: AgyMode::Plan,
        skip_permissions: true,
        extra_add_dir: None,
        resume_conversation: None,
        agy_bin: spec.agy_bin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_turn_skips_permissions_and_can_add_mcp_dir() {
        let plan = provider_turn_argv(ProviderTurnSpec {
            prompt: "hi",
            model: "gemini-3.6-flash",
            mode: AgyMode::AcceptEdits,
            skip_permissions: true,
            extra_add_dir: Some("/tmp/grog-agy-mcp"),
            resume_conversation: Some("abc"),
            agy_bin: None,
        });
        assert_eq!(plan.program, "agy");
        assert!(plan.args.contains(&"--dangerously-skip-permissions".into()));
        assert!(plan
            .args
            .windows(2)
            .any(|w| w == ["--add-dir", "/tmp/grog-agy-mcp"]));
        assert!(plan.args.windows(2).any(|w| w == ["--conversation", "abc"]));
    }

    #[test]
    fn ask_does_not_forward_mcp_dir() {
        let plan = ask_agy_argv(AskAntigravitySpec {
            prompt: "second opinion",
            model: "gemini-3.1-pro",
            agy_bin: Some("/usr/bin/agy"),
        });
        assert!(!plan.args.iter().any(|a| a == "--add-dir"));
        assert_eq!(plan.program, "/usr/bin/agy");
        assert!(plan.args.windows(2).any(|w| w == ["--mode", "plan"]));
        assert!(!plan
            .args
            .windows(2)
            .any(|w| w == ["--mode", "accept-edits"]));
    }
}
