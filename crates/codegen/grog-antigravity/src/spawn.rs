//! Argv for unmodified `agy`. Default skip-permissions so `-p` cannot hang
//! on an unanswerable y/n prompt (upstream antigravity-cli#318).
//!
//! Google's Go flag parser treats `-p`/`--print`/`--prompt` as a
//! **value-taking** flag: the next argv token is the prompt. Flags such as
//! `--model` must come *before* `-p`, and `-p` must be immediately followed by
//! the user query — never by `--model` or a model id.

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

/// The prompt token that `-p`/`--print`/`--prompt` will consume.
/// `None` if those flags are missing or not followed by a value.
pub fn prompt_from_args(args: &[String]) -> Option<&str> {
    let idx = args
        .iter()
        .rposition(|a| a == "-p" || a == "--print" || a == "--prompt")?;
    args.get(idx + 1).map(String::as_str)
}

pub fn provider_turn_argv(spec: ProviderTurnSpec<'_>) -> AgySpawnPlan {
    let mut args = Vec::new();
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
    // `-p` last: the Go parser would otherwise eat the next flag as the prompt.
    args.push("-p".into());
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

    fn print_flag_index(args: &[String]) -> usize {
        args.iter()
            .position(|a| a == "-p" || a == "--print" || a == "--prompt")
            .expect("-p/--print/--prompt")
    }

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
        assert!(
            plan.args
                .windows(2)
                .any(|w| w == ["--add-dir", "/tmp/grog-agy-mcp"])
        );
        assert!(plan.args.windows(2).any(|w| w == ["--conversation", "abc"]));
        assert_eq!(prompt_from_args(&plan.args), Some("hi"));
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
        assert!(
            !plan
                .args
                .windows(2)
                .any(|w| w == ["--mode", "accept-edits"])
        );
    }

    #[test]
    fn ask_prompt_is_the_user_query_not_model_flag() {
        let prompt = "should we cache invalidation?";
        let model = "gemini-3.6-flash";
        let plan = ask_agy_argv(AskAntigravitySpec {
            prompt,
            model,
            agy_bin: None,
        });

        let p = print_flag_index(&plan.args);
        assert!(
            p > 0,
            "-p must not be first; flags such as --model must precede it: {:?}",
            plan.args
        );
        assert_eq!(
            plan.args.get(p + 1).map(String::as_str),
            Some(prompt),
            "-p must be immediately followed by the user query, got {:?}",
            plan.args
        );
        assert_eq!(plan.args.last().map(String::as_str), Some(prompt));
        assert_eq!(prompt_from_args(&plan.args), Some(prompt));
        assert_ne!(prompt_from_args(&plan.args), Some("--model"));
        assert_ne!(prompt_from_args(&plan.args), Some(model));

        let model_idx = plan
            .args
            .iter()
            .position(|a| a == "--model")
            .expect("--model");
        assert!(
            model_idx < p,
            "--model must come before -p so the Go parser does not eat it as the prompt: {:?}",
            plan.args
        );
        assert_eq!(
            plan.args.get(model_idx + 1).map(String::as_str),
            Some(model)
        );
        assert!(
            plan.args[..p]
                .iter()
                .any(|a| a == "--dangerously-skip-permissions")
        );
        assert!(plan.args[..p].windows(2).any(|w| w == ["--mode", "plan"]));
        assert!(
            !plan.args[p + 1].starts_with('-'),
            "prompt slot must not be a flag, got {}",
            plan.args[p + 1]
        );
    }

    #[test]
    fn provider_turn_also_puts_print_last_before_prompt() {
        let plan = provider_turn_argv(ProviderTurnSpec {
            prompt: "edit the file",
            model: "gemini-3.6-flash",
            mode: AgyMode::AcceptEdits,
            skip_permissions: true,
            extra_add_dir: None,
            resume_conversation: None,
            agy_bin: None,
        });
        assert_eq!(prompt_from_args(&plan.args), Some("edit the file"));
        let p = print_flag_index(&plan.args);
        assert_eq!(
            plan.args.get(p + 1).map(String::as_str),
            Some("edit the file")
        );
        assert!(
            plan.args[..p]
                .windows(2)
                .any(|w| w == ["--mode", "accept-edits"])
        );
    }
}
