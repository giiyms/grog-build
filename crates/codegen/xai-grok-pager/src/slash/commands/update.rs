//! `/update` -- download the latest grog, then restart this session.
//!
//! Same install as `grog update` (GitHub Releases Darwin aarch64 into
//! `~/.grog`, never `~/.grok`). After a successful install, quit and
//! re-exec like [`super::restart::RestartCommand`].

use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand};

/// Fetch grog from GitHub Releases and reopen this session on the new binary.
pub struct UpdateCommand;

impl SlashCommand for UpdateCommand {
    fn name(&self) -> &str {
        "update"
    }

    fn aliases(&self) -> &[&str] {
        &["upgrade"]
    }

    fn description(&self) -> &str {
        "Update grog from GitHub Releases and resume this session"
    }

    fn usage(&self) -> &str {
        "/update"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn run(&self, ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        if ctx.session_id.is_none() {
            return CommandResult::Error("No active session to update and restart".to_string());
        }
        CommandResult::Action(Action::UpdateGrog)
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
        let desc = UpdateCommand.description();
        assert!(desc.contains("grog"), "/update help must name grog: {desc}");
        assert!(
            !desc.to_ascii_lowercase().contains("grok"),
            "/update help must not say grok: {desc}"
        );
    }

    #[test]
    fn run_returns_update_action_with_session() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let sid = agent_client_protocol::SessionId::from("sess-abc".to_string());
        let mut ctx = exec_ctx(&models, &bundle, Some(&sid));
        assert!(matches!(
            UpdateCommand.run(&mut ctx, ""),
            CommandResult::Action(Action::UpdateGrog)
        ));
    }

    #[test]
    fn run_errors_without_session() {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = exec_ctx(&models, &bundle, None);
        assert!(matches!(
            UpdateCommand.run(&mut ctx, ""),
            CommandResult::Error(msg) if msg.contains("No active session")
        ));
    }

    #[test]
    fn registered_in_builtin_commands() {
        let reg = CommandRegistry::new(crate::slash::commands::builtin_commands());
        let cmd = reg.get("update").expect("/update must be registered");
        assert_eq!(cmd.name(), "update");
        assert_eq!(cmd.description(), UpdateCommand.description());
        let alias = reg.get("upgrade").expect("/upgrade alias");
        assert_eq!(alias.name(), "update");
    }
}
