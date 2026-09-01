//! Argv builders for `claude` print-mode turns.
//!
//! The child uses whatever login already lives in `~/.claude`. Grog does not
//! pass an API key.

use crate::models::{AskClaudeMode, LongContextSettings, resolve_cli_model};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeSpawnPlan {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTurnSpec<'a> {
    pub prompt: &'a str,
    pub model_id: &'a str,
    pub settings: LongContextSettings,
    pub resume_session: Option<&'a str>,
    pub mcp_config_path: Option<&'a str>,
    pub claude_bin: Option<&'a str>,
    pub effort: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskClaudeSpec<'a> {
    pub prompt: &'a str,
    pub model_id: &'a str,
    pub settings: LongContextSettings,
    pub mode: AskClaudeMode,
    pub isolated: bool,
    pub claude_bin: Option<&'a str>,
    pub effort: Option<&'a str>,
}

fn print_args(model: &str, effort: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--model".into(),
        model.into(),
    ];
    if let Some(effort) = effort {
        args.extend(["--effort".into(), effort.into()]);
    }
    args
}

pub fn provider_turn_argv(spec: ProviderTurnSpec<'_>) -> ClaudeSpawnPlan {
    let cli = resolve_cli_model(spec.model_id, spec.settings);
    let mut args = print_args(cli.cli_id, spec.effort);
    if let Some(session) = spec.resume_session {
        args.extend(["--resume".into(), session.into()]);
    }
    if let Some(mcp) = spec.mcp_config_path {
        args.extend(["--mcp-config".into(), mcp.into()]);
        args.push("--strict-mcp-config".into());
    }
    args.push(spec.prompt.into());
    ClaudeSpawnPlan {
        program: spec.claude_bin.unwrap_or("claude").to_string(),
        args,
    }
}

pub fn ask_claude_argv(spec: AskClaudeSpec<'_>) -> ClaudeSpawnPlan {
    let cli = resolve_cli_model(spec.model_id, spec.settings);
    let effort = spec.effort.or(Some(crate::DEFAULT_CLAUDE_EFFORT));
    let mut args = print_args(cli.cli_id, effort);
    match spec.mode {
        AskClaudeMode::None => args.extend(["--tools".into(), "".into()]),
        AskClaudeMode::Read => args.extend([
            "--allowedTools".into(),
            "Read,Glob,Grep,WebSearch,WebFetch".into(),
        ]),
        AskClaudeMode::Full => {}
    }
    if spec.isolated {
        args.push("--no-session-persistence".into());
    }
    args.push(spec.prompt.into());
    ClaudeSpawnPlan {
        program: spec.claude_bin.unwrap_or("claude").to_string(),
        args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LongContextSettings;

    #[test]
    fn provider_turn_uses_stream_json_and_1m_suffix() {
        let plan = provider_turn_argv(ProviderTurnSpec {
            prompt: "hello",
            model_id: "claude-opus-4-8",
            settings: LongContextSettings::default(),
            resume_session: None,
            mcp_config_path: None,
            claude_bin: None,
            effort: None,
        });
        assert_eq!(plan.program, "claude");
        assert!(
            plan.args
                .windows(2)
                .any(|w| w == ["--model", "claude-opus-4-8[1m]"])
        );
        assert!(
            plan.args
                .windows(2)
                .any(|w| w == ["--output-format", "stream-json"])
        );
        assert_eq!(plan.args.last().unwrap(), "hello");
        let typed = provider_turn_argv(ProviderTurnSpec {
            prompt: "hello",
            model_id: "claude-fable-5-1",
            settings: LongContextSettings::default(),
            resume_session: None,
            mcp_config_path: None,
            claude_bin: None,
            effort: None,
        });
        assert!(
            typed
                .args
                .windows(2)
                .any(|w| w == ["--model", "claude-fable-5-1"])
        );
        let unknown = provider_turn_argv(ProviderTurnSpec {
            prompt: "hello",
            model_id: "claude-not-in-catalog-yet",
            settings: LongContextSettings::default(),
            resume_session: None,
            mcp_config_path: None,
            claude_bin: None,
            effort: None,
        });
        assert!(
            unknown
                .args
                .windows(2)
                .any(|w| w == ["--model", "claude-not-in-catalog-yet"]),
            "unknown ids must pass through, not rewrite to sonnet-4-6: {:?}",
            unknown.args
        );
    }

    #[test]
    fn ask_read_mode_restricts_tools() {
        let plan = ask_claude_argv(AskClaudeSpec {
            prompt: "review this",
            model_id: "claude-haiku-4-5",
            settings: LongContextSettings::default(),
            mode: AskClaudeMode::Read,
            isolated: true,
            claude_bin: Some("/opt/claude"),
            effort: None,
        });
        assert_eq!(plan.program, "/opt/claude");
        assert!(plan.args.iter().any(|a| a == "--allowedTools"));
        assert!(plan.args.iter().any(|a| a == "--no-session-persistence"));
    }

    #[test]
    fn ask_fable_51_uses_medium_effort_before_prompt() {
        let plan = ask_claude_argv(AskClaudeSpec {
            prompt: "second opinion",
            model_id: crate::DEFAULT_CLAUDE_MODEL,
            settings: LongContextSettings::default(),
            mode: AskClaudeMode::Read,
            isolated: true,
            claude_bin: None,
            effort: Some(crate::DEFAULT_CLAUDE_EFFORT),
        });
        assert!(
            plan.args
                .windows(2)
                .any(|w| w == ["--model", "claude-fable-5-1"])
        );
        assert!(
            !plan.args
                .windows(2)
                .any(|w| w == ["--model", "claude-sonnet-4-6"]),
            "Fable 5.1 must not be rewritten to Sonnet 4.6: {:?}",
            plan.args
        );
        assert!(plan.args.windows(2).any(|w| w == ["--effort", "medium"]));
        assert_eq!(plan.args.last().unwrap(), "second opinion");
        let effort = plan.args.iter().position(|a| a == "--effort").unwrap();
        let prompt = plan.args.len() - 1;
        assert!(
            effort < prompt,
            "--effort must precede the prompt: {:?}",
            plan.args
        );
        assert!(
            !plan.args.windows(2).any(|w| w == ["--effort", "high"]),
            "Fable 5.1 council/Ask default is medium, not Claude Code's High: {:?}",
            plan.args
        );
        assert!(
            !plan.args.windows(2).any(|w| w == ["--effort", "xhigh"]),
            "Fable 5.1 council/Ask default is medium, not a higher tier: {:?}",
            plan.args
        );
    }
}
