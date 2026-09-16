//! `/model` (alias `/m`): switch the model and optionally its reasoning effort.
//! Chained autocomplete: after picking a reasoning-supported model, the trailing space re-opens the dropdown into a `low|medium|high|xhigh` sub-menu.

use agent_client_protocol as acp;
use xai_grok_shell::sampling::types::supports_reasoning_effort_meta;

use crate::acp::model_state::ModelState;
use crate::app::actions::Action;
use crate::slash::command::{
    AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand, slash_meta,
};
use crate::slash::commands::effort_levels::build_effort_arg_items;
use crate::views::modal::ModelPickerTarget;
use grog_providers::source_label;
use grog_providers::visibility::{self, HiddenSet};

/// Switch the active model (and optionally its reasoning effort).
pub struct ModelCommand;

impl SlashCommand for ModelCommand {
    slash_meta! {
        name: "model",
        aliases: ["m", "models"],
        description: "Switch the active model",
        usage: "/model [hide|unhide|hidden|show-hidden|<name>] [effort]",
        takes_args: true,
        args_required: false,
        session_scoped: true,
        // The dashboard offers `/model` to pick the model for the next spawned agent (intercepted in `dispatch_dashboard_dispatch_slash`).
        offered_when_session_less: true,
        arg_placeholder: "<model> [effort]",
    }

    fn suggest_args(&self, ctx: &AppCtx, args_query: &str) -> Option<Vec<ArgItem>> {
        if ctx.models.is_empty() {
            return None;
        }

        // Effort phase if input is "<reasoning-model> ", else model phase.
        if let Some(model_id) = detect_effort_phase(ctx.models, args_query) {
            return Some(build_effort_items(ctx.models, &model_id));
        }
        Some(build_model_items(ctx.models))
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return CommandResult::Action(Action::OpenModelPicker {
                target: ModelPickerTarget::Session,
            });
        }

        let (verb, rest) = split_leading_verb(trimmed);
        match verb {
            "hide" => return hide_model(ctx.models, rest),
            "unhide" => return unhide_model(ctx.models, rest),
            "hidden" => return list_hidden(),
            "show-hidden" | "showhidden" => {
                let on = visibility::toggle_show_hidden();
                return CommandResult::Message(if on {
                    "Showing hidden models in the picker.".into()
                } else {
                    "Hidden models omitted from the picker.".into()
                });
            }
            _ => {}
        }

        // Prefer an exact full-string catalog match first. Model display names often contain spaces ("Grok 4.5").
        // If we split on the last token first, a shorter catalog entry ("Grok") would steal the prefix and treat "4.5" as an effort level
        if let Some(id) = ctx.models.resolve_by_name_or_id(trimmed) {
            return CommandResult::Action(Action::SetDefaultModel(id));
        }

        // A trailing effort token on a reasoning model makes a session-scoped switch (not persisted as default)
        // Resolve via the shared gate so a rejected level (e.g. `none` on grok-4.5) reports the effort error with the model's offered ids.
        // Without it the fall-through reports "Unknown model: … none"
        if let Some((prefix, token)) = split_trailing_token(trimmed)
            && let Some(id) = resolve_model(ctx.models, prefix)
            && ctx
                .models
                .available
                .get(&id)
                .map(supports_reasoning_effort)
                .unwrap_or(false)
        {
            return match ctx.models.resolve_effort_for_model(&id, token) {
                Ok(effort) => CommandResult::Action(Action::SwitchModel {
                    model_id: id,
                    effort: Some(effort),
                }),
                Err(err) => CommandResult::Error(err.message()),
            };
        }

        CommandResult::Error(format!("Unknown model: {trimmed}"))
    }
}

/// Look up a model by case-insensitive display name OR model id match.
fn resolve_model(models: &ModelState, name: &str) -> Option<acp::ModelId> {
    models.resolve_by_name_or_id(name)
}

fn supports_reasoning_effort(info: &acp::ModelInfo) -> bool {
    supports_reasoning_effort_meta(info.meta.as_ref())
}

/// Split `args` into `(prefix, last_token)` on the final whitespace run.
/// Returns `None` when there is no interior whitespace to split on.
/// The token is resolved to an effort against the picked model's options by the caller.
fn split_trailing_token(args: &str) -> Option<(&str, &str)> {
    let (prefix, last) = args.rsplit_once(char::is_whitespace)?;
    let prefix = prefix.trim_end();
    if prefix.is_empty() || last.is_empty() {
        return None;
    }
    Some((prefix, last))
}

/// Returns the matched model id when `args_query` is `"<reasoning-model> ..."`.
/// Candidates are tried longest name first to disambiguate names that share a prefix.
fn detect_effort_phase(models: &ModelState, args_query: &str) -> Option<acp::ModelId> {
    let mut candidates: Vec<(&acp::ModelId, &str)> = models
        .available
        .iter()
        .filter(|(_, info)| supports_reasoning_effort(info))
        .map(|(id, info)| (id, info.name.as_str()))
        .collect();
    candidates.sort_by_key(|(_, name)| std::cmp::Reverse(name.len()));

    for (id, name) in candidates {
        if args_query
            .get(..name.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
            && args_query
                .get(name.len()..)
                .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            return Some(id.clone());
        }
    }
    None
}

/// One row per logical model.
/// Reasoning models get a trailing space in `insert_text` so the prompt widget chains into the effort sub-menu.
fn build_model_items(models: &ModelState) -> Vec<ArgItem> {
    build_model_items_filtered(
        models,
        &visibility::load_hidden_from_grog_home(),
        visibility::show_hidden(),
    )
}

pub(crate) fn build_model_items_filtered(
    models: &ModelState,
    hidden: &HiddenSet,
    show_hidden: bool,
) -> Vec<ArgItem> {
    let current_id = models.current.as_ref();
    let mut items: Vec<ArgItem> = Vec::with_capacity(models.available.len());
    for (id, info) in &models.available {
        let catalog_key = id.0.as_ref();
        let is_hidden = hidden.contains(catalog_key);
        if is_hidden && !show_hidden {
            continue;
        }
        let source = source_label(catalog_key);
        let is_current = current_id == Some(id);
        let supports = supports_reasoning_effort(info);

        let mut display = if is_current {
            format!("{} (current)", info.name)
        } else {
            info.name.clone()
        };
        if is_hidden {
            display.push_str(" (hidden)");
        }

        // A trailing space on reasoning models signals "more input expected" to the prompt widget
        // Enter then advances to the effort phase instead of submitting
        let insert_text = if supports {
            format!("{} ", info.name)
        } else {
            info.name.clone()
        };

        items.push(ArgItem {
            display,
            match_text: format!("{} {} {catalog_key}", info.name, source),
            insert_text,
            description: source.to_string(),
        });
    }
    items
}

fn split_leading_verb(args: &str) -> (&str, &str) {
    match args.split_once(char::is_whitespace) {
        Some((verb, rest)) => (verb, rest.trim()),
        None => (args, ""),
    }
}

fn hide_model(models: &ModelState, raw: &str) -> CommandResult {
    let Some(id) = resolve_hide_target(models, raw) else {
        return CommandResult::Error("Usage: /model hide <name>".into());
    };
    let key = id.0.to_string();
    let mut hidden = visibility::load_hidden_from_grog_home();
    hidden.hide(&key);
    if let Err(err) = visibility::persist_hidden_to_grog_home(&hidden) {
        return CommandResult::Error(format!("could not persist hidden models: {err}"));
    }
    CommandResult::Message(format!("Hid {key} from the picker."))
}

fn unhide_model(models: &ModelState, raw: &str) -> CommandResult {
    let key = if let Some(id) = resolve_hide_target(models, raw) {
        id.0.to_string()
    } else if !raw.is_empty() {
        raw.to_string()
    } else {
        return CommandResult::Error("Usage: /model unhide <name>".into());
    };
    let mut hidden = visibility::load_hidden_from_grog_home();
    hidden.unhide(&key);
    if let Err(err) = visibility::persist_hidden_to_grog_home(&hidden) {
        return CommandResult::Error(format!("could not persist hidden models: {err}"));
    }
    CommandResult::Message(format!("Unhid {key}."))
}

fn list_hidden() -> CommandResult {
    let hidden = visibility::load_hidden_from_grog_home();
    if hidden.is_empty() {
        return CommandResult::Message("No hidden models.".into());
    }
    let mut lines = String::from("Hidden models:\n");
    for id in hidden.iter() {
        lines.push_str("  ");
        lines.push_str(id);
        lines.push('\n');
    }
    lines.push_str("Unhide with /model unhide <id>, or /model show-hidden to preview.");
    CommandResult::Message(lines)
}

fn resolve_hide_target(models: &ModelState, raw: &str) -> Option<acp::ModelId> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    models.resolve_by_name_or_id(trimmed)
}

/// One row per effort level for the `/model` chained effort phase.
/// `insert_text` is `"ModelName high"` so selecting a row completes both tokens.
fn build_effort_items(models: &ModelState, model_id: &acp::ModelId) -> Vec<ArgItem> {
    let info = match models.available.get(model_id) {
        Some(info) => info,
        None => return Vec::new(),
    };
    let model_name = info.name.clone();
    let is_current_model = models.current.as_ref() == Some(model_id);
    let options = models.reasoning_effort_options_for(model_id);
    build_effort_arg_items(
        &options,
        models.reasoning_effort,
        is_current_model,
        |option| format!("{model_name} {}", option.id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use xai_grok_shell::sampling::types::ReasoningEffort;

    fn model_with_reasoning(id: &str, name: &str) -> (acp::ModelId, acp::ModelInfo) {
        let id = acp::ModelId::new(Arc::from(id));
        let mut meta = serde_json::Map::new();
        meta.insert(
            "supportsReasoningEffort".into(),
            serde_json::Value::Bool(true),
        );
        let info = acp::ModelInfo::new(id.clone(), name.to_string())
            .meta(serde_json::Value::Object(meta).as_object().cloned());
        (id, info)
    }

    fn plain_model(id: &str, name: &str) -> (acp::ModelId, acp::ModelInfo) {
        let id = acp::ModelId::new(Arc::from(id));
        let info = acp::ModelInfo::new(id.clone(), name.to_string());
        (id, info)
    }

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
            pager_state: crate::settings::PagerLocalSnapshot {
                multiline_mode: false,
                yolo_mode: false,
                ..crate::settings::PagerLocalSnapshot::default()
            },
        }
    }

    #[test]
    fn split_trailing_token_splits_on_final_whitespace() {
        assert_eq!(
            split_trailing_token("Reasoning X high"),
            Some(("Reasoning X", "high"))
        );
        assert_eq!(
            split_trailing_token("reasoning-x  xhigh"),
            Some(("reasoning-x", "xhigh"))
        );
        // No interior whitespace, so nothing to split off
        assert!(split_trailing_token("reasoning-x-pro").is_none());
    }

    #[test]
    fn empty_query_returns_one_row_per_logical_model() {
        let mut state = ModelState::default();
        let (rid, rinfo) = model_with_reasoning("reasoning-x", "Reasoning X");
        let (pid, pinfo) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(rid, rinfo);
        state.available.insert(pid, pinfo);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: true,
            saved_workflows: &[],
            workflow_runs: &[],
            screen_mode: crate::app::ScreenMode::Fullscreen,
            current_title: None,
        };
        let items = cmd.suggest_args(&ctx, "").unwrap();
        assert_eq!(items.len(), 2, "model phase: one row per logical model");

        // A reasoning model has a trailing space in insert_text
        // The prompt widget reads it to keep the dropdown open after Enter so the effort sub-menu can render
        let reasoning = items
            .iter()
            .find(|i| i.match_text.contains("Reasoning X"))
            .unwrap();
        assert_eq!(reasoning.insert_text, "Reasoning X ");

        // A plain model has no trailing space, so Enter commits immediately
        let plain = items
            .iter()
            .find(|i| i.match_text.contains("Grok 4.5"))
            .unwrap();
        assert_eq!(plain.insert_text, "Grok 4.5");
        assert_eq!(plain.description, "Grok");
    }

    #[test]
    fn trailing_space_after_reasoning_model_enters_effort_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: true,
            saved_workflows: &[],
            workflow_runs: &[],
            screen_mode: crate::app::ScreenMode::Fullscreen,
            current_title: None,
        };
        // The args query has a trailing space, so this is the effort phase
        // Items come out ordered xhigh to low (strongest first) per EFFORT_LEVELS
        let items = cmd.suggest_args(&ctx, "Reasoning X ").unwrap();
        assert_eq!(items.len(), 4);
        let [a, b, c, d] = items.as_slice() else {
            panic!("expected 4 items: {items:?}");
        };
        assert_eq!(a.insert_text, "Reasoning X xhigh");
        assert_eq!(b.insert_text, "Reasoning X high");
        assert_eq!(c.insert_text, "Reasoning X medium");
        assert_eq!(d.insert_text, "Reasoning X low");
        // Display is just the level so the user sees a clean column.
        assert_eq!(a.display, "xhigh");
        // match_text carries the sort-key prefix that forces the matcher's alphabetical tiebreak to render rows in EFFORT_LEVELS order
        assert!(a.match_text.starts_with("a "));
        assert!(d.match_text.starts_with("d "));
    }

    #[test]
    fn partial_effort_query_still_in_effort_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: true,
            saved_workflows: &[],
            workflow_runs: &[],
            screen_mode: crate::app::ScreenMode::Fullscreen,
            current_title: None,
        };
        // Still in effort phase; the matcher upstream narrows to high and xhigh
        let items = cmd.suggest_args(&ctx, "Reasoning X h").unwrap();
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn partial_model_query_stays_in_model_phase() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);

        let cmd = ModelCommand;
        let ctx = AppCtx {
            models: &state,
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: true,
            saved_workflows: &[],
            workflow_runs: &[],
            screen_mode: crate::app::ScreenMode::Fullscreen,
            current_title: None,
        };
        // No trailing space: the user is still typing the model name
        let items = cmd.suggest_args(&ctx, "Reason").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(
            items.first().map(|item| item.insert_text.as_str()),
            Some("Reasoning X ")
        );
    }

    #[test]
    fn run_parses_model_plus_effort_when_supported() {
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Reasoning X xhigh");
        match result {
            CommandResult::Action(Action::SwitchModel { model_id, effort }) => {
                assert_eq!(model_id.0.as_ref(), "reasoning-x");
                assert_eq!(effort, Some(ReasoningEffort::Xhigh));
            }
            other => panic!("expected SwitchModel with effort, got {other:?}"),
        }
    }

    #[test]
    fn run_rejects_unoffered_effort_with_effort_error_not_unknown_model() {
        // Regression: previously `resolve_effort_token_for` returned None and the handler fell through to `Unknown model: Reasoning X none`
        let mut state = ModelState::default();
        let (id, info) = model_with_reasoning("reasoning-x", "Reasoning X");
        state.available.insert(id, info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Reasoning X none");
        match result {
            CommandResult::Error(msg) => {
                assert!(
                    msg.contains("unknown effort level 'none'"),
                    "expected effort error, got {msg}"
                );
                assert!(
                    msg.contains("use one of:"),
                    "expected offered levels in message, got {msg}"
                );
                assert!(
                    !msg.to_lowercase().contains("unknown model"),
                    "must not misreport as unknown model: {msg}"
                );
                let offered = msg.split_once("; ").map(|(_, r)| r).unwrap_or("");
                assert!(
                    !offered.contains("none"),
                    "must not list none as offered: {msg}"
                );
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn run_prefers_full_multi_word_model_name_over_prefix_plus_effort() {
        // The catalog has both "Grok" (reasoning) and "Grok 4.5"
        // `/model Grok 4.5` must select the full name, not treat "4.5" as an effort on "Grok"
        let mut state = ModelState::default();
        let (short_id, short_info) = model_with_reasoning("grok", "Grok");
        let (long_id, long_info) = model_with_reasoning("grok-4.5", "Grok 4.5");
        state.available.insert(short_id, short_info);
        state.available.insert(long_id.clone(), long_info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Grok 4.5");
        match result {
            CommandResult::Action(Action::SetDefaultModel(resolved_id)) => {
                assert_eq!(resolved_id, long_id);
            }
            other => panic!("expected SetDefaultModel(Grok 4.5), got {other:?}"),
        }
    }

    #[test]
    fn run_rejects_effort_for_non_reasoning_model() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(id, info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Grok 4.5 high");
        // Falls through to "is the whole string a model name?", which it isn't, so we get an Unknown error
        assert!(matches!(result, CommandResult::Error(_)));
    }

    /// The bare `/model <name>` form dispatches `Action::SetDefaultModel(<ModelId>)` instead of the legacy `Action::SwitchModel { effort: None }`.
    /// The dispatcher routes it through both `Effect::SwitchModel` (session mutation) and `Effect::PersistSetting` (next-session default).
    /// The payload is the typed `acp::ModelId` (resolved at the slash boundary), not a String.
    #[test]
    fn run_bare_model_name_dispatches_set_default_model() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(id.clone(), info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "Grok 4.5");
        match result {
            CommandResult::Action(Action::SetDefaultModel(resolved_id)) => {
                assert_eq!(resolved_id, id);
            }
            other => panic!("expected Action::SetDefaultModel(<id>), got {other:?}"),
        }
    }

    /// Case-insensitive matching against the catalog: `/model grok 4.5` resolves to the same `ModelId` as `/model Grok 4.5`.
    #[test]
    fn run_set_default_model_resolves_case_insensitively() {
        let mut state = ModelState::default();
        let (id, info) = plain_model("grok-4.5", "Grok 4.5");
        state.available.insert(id.clone(), info);
        let mut ctx = dummy_exec_ctx(&state);
        let result = ModelCommand.run(&mut ctx, "grok 4.5");
        match result {
            CommandResult::Action(Action::SetDefaultModel(resolved_id)) => {
                assert_eq!(resolved_id, id);
            }
            other => panic!("expected Action::SetDefaultModel(<id>), got {other:?}"),
        }
    }

    #[test]
    fn picker_rows_include_source_labels() {
        let mut state = ModelState::default();
        let (gid, ginfo) = plain_model("grok-4.6", "Grok 4.6");
        let (cid, cinfo) = plain_model("claude-bridge/claude-fable-5-1", "Fable 5.1");
        let (aid, ainfo) =
            plain_model("antigravity/gemini-3.8-flash-high", "Gemini 3.8 Flash High");
        let (xid, xinfo) = plain_model("codex/gpt-5.6-luna", "GPT-5.6 Luna");
        let (oid, oinfo) = plain_model("local-llama", "Local Llama");
        state.available.insert(gid, ginfo);
        state.available.insert(cid, cinfo);
        state.available.insert(aid, ainfo);
        state.available.insert(xid, xinfo);
        state.available.insert(oid, oinfo);
        let hidden = HiddenSet::from_text("");
        let items = build_model_items_filtered(&state, &hidden, false);
        let desc = |name: &str| {
            items
                .iter()
                .find(|i| i.display == name)
                .map(|i| i.description.as_str())
                .unwrap_or("")
        };
        assert_eq!(desc("Grok 4.6"), "Grok");
        assert_eq!(desc("Fable 5.1"), "Claude");
        assert_eq!(desc("Gemini 3.8 Flash High"), "Antigravity");
        assert_eq!(desc("GPT-5.6 Luna"), "Codex");
        assert_eq!(desc("Local Llama"), "Custom");
        assert!(items.iter().any(|i| i.match_text.contains("Antigravity")
            && i.match_text.contains("gemini-3.8-flash-high")));
    }

    #[test]
    fn hide_filters_picker_and_unhide_restores() {
        let mut state = ModelState::default();
        let (keep_id, keep_info) = plain_model("grok-4.6", "Grok 4.6");
        let (hide_id, hide_info) = plain_model("antigravity/gemini-3.6-flash", "Gemini 3.6 Flash");
        state.available.insert(keep_id, keep_info);
        state.available.insert(hide_id, hide_info);
        let mut hidden = HiddenSet::from_text("");
        hidden.hide("antigravity/gemini-3.6-flash");
        let filtered = build_model_items_filtered(&state, &hidden, false);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].display, "Grok 4.6");
        let preview = build_model_items_filtered(&state, &hidden, true);
        assert_eq!(preview.len(), 2);
        assert!(
            preview
                .iter()
                .any(|i| i.display.contains("(hidden)") && i.description == "Antigravity")
        );
        hidden.unhide("antigravity/gemini-3.6-flash");
        let restored = build_model_items_filtered(&state, &hidden, false);
        assert_eq!(restored.len(), 2);
    }

    #[test]
    fn empty_model_command_opens_session_picker() {
        let state = ModelState::default();
        let mut ctx = dummy_exec_ctx(&state);
        match ModelCommand.run(&mut ctx, "") {
            CommandResult::Action(Action::OpenModelPicker { target }) => {
                assert_eq!(target, ModelPickerTarget::Session);
            }
            other => panic!("expected OpenModelPicker, got {other:?}"),
        }
    }
}
