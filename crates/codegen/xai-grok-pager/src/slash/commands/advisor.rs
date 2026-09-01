//! `/advisor` — pager half of the sidecar reviewer.
//!
//! `/advisor model` opens the shared model picker targeting Advisor so
//! picking Luna does not switch the live primary. Named models persist
//! `models.advisor` and pass through to the shell to enable. Toggle /
//! status / dump stay shell-owned.

use grog_advisor::{AdvisorVerb, parse_verb, resolve_spec};

use crate::app::actions::Action;
use crate::slash::command::{AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand};
use crate::slash::commands::model::ModelCommand;
use crate::views::modal::ModelPickerTarget;

/// Session-scoped sidecar reviewer. Picker + persist live here; enable
/// and consult live in the shell.
pub struct AdvisorCommand;

impl SlashCommand for AdvisorCommand {
    fn name(&self) -> &str {
        "advisor"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn description(&self) -> &str {
        "Sidecar reviewer (toggle, pick model, status)"
    }

    fn session_scoped(&self) -> bool {
        true
    }

    fn offered_when_session_less(&self) -> bool {
        false
    }

    fn usage(&self) -> &str {
        "/advisor [on|off|status|model|luna|fable|opus|sonnet|agy]"
    }

    fn takes_args(&self) -> bool {
        true
    }

    fn args_required(&self) -> bool {
        false
    }

    fn arg_placeholder(&self) -> Option<&str> {
        Some("[on|off|status|model|<name>]")
    }

    fn suggest_args(&self, ctx: &AppCtx, args_query: &str) -> Option<Vec<ArgItem>> {
        // Same catalog as `/model` so the overlay is one widget, two slots.
        ModelCommand.suggest_args(ctx, args_query)
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let verb = match parse_verb(args) {
            Ok(v) => v,
            Err(e) => return CommandResult::Error(e),
        };
        match verb {
            AdvisorVerb::OpenPicker => CommandResult::Action(Action::OpenModelPicker {
                target: ModelPickerTarget::Advisor,
            }),
            AdvisorVerb::Set(spec) | AdvisorVerb::On { spec: Some(spec) } => {
                persist_and_enable(ctx, &spec.raw, spec.effort.as_deref())
            }
            AdvisorVerb::Toggle
            | AdvisorVerb::On { spec: None }
            | AdvisorVerb::Off
            | AdvisorVerb::Status
            | AdvisorVerb::Dump
            | AdvisorVerb::Cycle => {
                let trimmed = args.trim();
                if trimmed.is_empty() {
                    CommandResult::PassThrough("/advisor".into())
                } else {
                    CommandResult::PassThrough(format!("/advisor {trimmed}"))
                }
            }
        }
    }
}

fn persist_and_enable(ctx: &CommandExecCtx<'_>, raw: &str, effort: Option<&str>) -> CommandResult {
    if let Ok(seat) = resolve_spec(raw, effort) {
        let id = ctx
            .models
            .resolve_by_name_or_id(&seat.qualified)
            .or_else(|| ctx.models.resolve_by_name_or_id(raw));
        let Some(id) = id else {
            // Catalog may not list native ids until login; still pass through.
            let pass = match effort {
                Some(e) => format!("/advisor {} {e}", seat.qualified),
                None => format!("/advisor {}", seat.qualified),
            };
            return CommandResult::PassThrough(pass);
        };
        let pass = match effort {
            Some(e) => format!("/advisor {} {e}", id.0),
            None => format!("/advisor {}", id.0),
        };
        return CommandResult::Action(Action::AdvisorSetAndEnable {
            model_id: id,
            pass_through: pass,
        });
    }

    if let Some(id) = ctx.models.resolve_by_name_or_id(raw) {
        let pass = match effort {
            Some(e) => format!("/advisor {} {e}", id.0),
            None => format!("/advisor {}", id.0),
        };
        return CommandResult::Action(Action::AdvisorSetAndEnable {
            model_id: id,
            pass_through: pass,
        });
    }

    CommandResult::Error(format!(
        "unknown advisor model '{raw}'. Try luna, fable, opus, sonnet, agy, or /advisor model"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::slash::command::CommandExecCtx;
    use agent_client_protocol as acp;
    use std::sync::Arc;

    static EMPTY_BUNDLE: crate::app::bundle::BundleState = crate::app::bundle::BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn dummy_exec_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &EMPTY_BUNDLE,
            screen_mode: crate::app::ScreenMode::Inline,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: crate::settings::PagerLocalSnapshot::default(),
        }
    }

    fn models_with_luna() -> ModelState {
        let mut models = ModelState::default();
        let id = acp::ModelId::new(Arc::from("codex/gpt-5.6-luna"));
        models.available.insert(
            id.clone(),
            acp::ModelInfo::new(id, "GPT-5.6 Luna".to_string()),
        );
        models
    }

    #[test]
    fn bare_model_opens_advisor_picker_not_cycle() {
        let models = models_with_luna();
        let mut ctx = dummy_exec_ctx(&models);
        match AdvisorCommand.run(&mut ctx, "model") {
            CommandResult::Action(Action::OpenModelPicker { target }) => {
                assert_eq!(target, ModelPickerTarget::Advisor);
            }
            other => panic!("expected OpenModelPicker Advisor, got {other:?}"),
        }
    }

    #[test]
    fn luna_persists_advisor_slot_without_set_default_model() {
        let models = models_with_luna();
        let mut ctx = dummy_exec_ctx(&models);
        match AdvisorCommand.run(&mut ctx, "luna") {
            CommandResult::Action(Action::AdvisorSetAndEnable {
                model_id,
                pass_through,
            }) => {
                assert_eq!(model_id.0.as_ref(), "codex/gpt-5.6-luna");
                assert!(pass_through.starts_with("/advisor "), "{pass_through}");
            }
            CommandResult::Action(Action::SetDefaultModel(_)) => {
                panic!("picking advisor must not switch the live primary");
            }
            other => panic!("expected AdvisorSetAndEnable, got {other:?}"),
        }
    }

    #[test]
    fn toggle_and_status_pass_through() {
        let models = models_with_luna();
        let mut ctx = dummy_exec_ctx(&models);
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, ""),
            CommandResult::PassThrough(t) if t == "/advisor"
        ));
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, "status"),
            CommandResult::PassThrough(t) if t == "/advisor status"
        ));
        assert!(matches!(
            AdvisorCommand.run(&mut ctx, "off"),
            CommandResult::PassThrough(t) if t == "/advisor off"
        ));
    }

    #[test]
    fn family_aliases_and_on_luna_enable_without_set_default_model() {
        let models = models_with_luna();
        let mut ctx = dummy_exec_ctx(&models);
        for args in ["luna", "on luna", "codex/gpt-5.6-luna"] {
            match AdvisorCommand.run(&mut ctx, args) {
                CommandResult::Action(Action::AdvisorSetAndEnable { model_id, .. }) => {
                    assert_eq!(model_id.0.as_ref(), "codex/gpt-5.6-luna", "args={args}");
                }
                CommandResult::Action(Action::SetDefaultModel(_)) => {
                    panic!("advisor alias {args} must not switch the live primary");
                }
                other => panic!("expected AdvisorSetAndEnable for {args}, got {other:?}"),
            }
        }
    }
}
