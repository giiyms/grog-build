//! `/restart` -- quit grog and immediately reopen the same session.
//!
//! Reuses the existing `--resume <session-id>` path (same as `/minimal`
//! exec fallback): persist is the normal session log under `$GROG_HOME`,
//! the TUI exits like `/exit`, then this process is replaced with the
//! running grog binary (never a hardcoded `~/.grok/bin/grok`).
//!
//! An in-flight agent turn is cancelled first so tool children are reaped
//! before the TTY is handed to the new process. We do not wait for the model
//! to finish; resume is from the last flushed checkpoint.

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Quit and reopen grog on the active session.
pub struct RestartCommand;

impl SlashCommand for RestartCommand {
    fn name(&self) -> &str {
        "restart"
    }

    fn description(&self) -> &str {
        "Restart grog and resume this session"
    }

    fn usage(&self) -> &str {
        "/restart"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("No active session to restart".to_string());
        }
        CommandResult::Action(Action::RestartProcess)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::slash::registry::CommandRegistry;

    fn exec_ctx<'a>(
        models: &'a ModelState,
        bundle: &'a BundleState,
        session: Option<&'a agent_client_protocol::SessionId>,
    ) -> CommandExecCtx<'a> {
        CommandExecCtx {
            models,
            session_id: session,
            bundle_state: bundle,
            screen_mode: crate::app::ScreenMode::Fullscreen,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn description_is_grog_not_grok() {
        let desc = RestartCommand.description();
        assert!(
            desc.contains("grog"),
            "/restart help must name grog: {desc}"
        );
        assert!(
            !desc.to_ascii_lowercase().contains("grok"),
            "/restart help must not say grok: {desc}"
        );
    }

    #[test]
    fn run_returns_restart_action_with_session() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let sid = agent_client_protocol::SessionId::from("sess-abc".to_string());
        let mut ctx = exec_ctx(&models, &bundle, Some(&sid));
        assert!(matches!(
            RestartCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::RestartProcess)
        ));
    }

    #[test]
    fn run_errors_without_session() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = exec_ctx(&models, &bundle, None);
        assert!(matches!(
            RestartCommand.run(&mut ctx, ""),
            CommandResult::Error(msg) if msg.contains("No active session")
        ));
    }

    #[test]
    fn registered_in_builtin_commands() {
        let reg = CommandRegistry::new(crate::slash::commands::builtin_commands());
        let cmd = reg.get("restart").expect("/restart must be registered");
        assert_eq!(cmd.name(), "restart");
        assert_eq!(cmd.description(), RestartCommand.description());
    }
}
