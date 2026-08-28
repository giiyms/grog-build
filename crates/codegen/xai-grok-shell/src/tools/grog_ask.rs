//! AskClaude / AskAntigravity / AskCodex consult tools.
//!
//! Dual surface with `/model`: when the session is already on that provider,
//! the matching Ask* tool refuses so council fan-out cannot recurse.

use std::cell::RefCell;
use std::sync::Once;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use xai_grok_tools::types::output::ToolOutput;
use xai_grok_tools::types::tool::{ToolKind, ToolNamespace};
use xai_grok_tools::types::tool_io::ToolInput;
use xai_grok_tools::types::tool_metadata::ToolMetadata;

thread_local! {
    static SESSION_MODEL: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_session_model(model: Option<String>) {
    SESSION_MODEL.with(|slot| *slot.borrow_mut() = model);
}

fn session_model() -> Option<String> {
    SESSION_MODEL.with(|slot| slot.borrow().clone())
}

fn session_is(provider: grog_providers::ProviderId) -> bool {
    session_model().is_some_and(|id| grog_providers::ModelRef::parse(&id).provider == provider)
}

pub fn ensure_registered() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        xai_grok_tools::registry::types::register_tool_pack(register_ask_tools);
    });
}

fn register_ask_tools(builder: &mut xai_grok_tools::registry::types::ToolRegistryBuilder) {
    builder.register::<AskClaudeTool>();
    builder.register::<AskAntigravityTool>();
    builder.register::<AskCodexTool>();
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AskConsultInput {
    /// Question or brief to send to the other model.
    pub prompt: String,
    /// Optional catalog id (e.g. `claude-opus-5`). Default is the provider's first catalog entry.
    #[serde(default)]
    pub model: Option<String>,
}

impl From<AskConsultInput> for ToolInput {
    fn from(value: AskConsultInput) -> Self {
        ToolInput::Dynamic(serde_json::to_value(value).unwrap_or_default())
    }
}

macro_rules! ask_tool {
    ($tool:ident, $id:literal, $desc:literal, $provider:expr, $default_model:expr) => {
        #[derive(Debug, Default)]
        pub struct $tool;

        impl ToolMetadata for $tool {
            fn kind(&self) -> ToolKind {
                ToolKind::Other
            }

            fn tool_namespace(&self) -> ToolNamespace {
                ToolNamespace::GrokBuild
            }

            fn description_template(&self) -> &str {
                $desc
            }

            fn is_read_only(&self) -> bool {
                true
            }
        }

        impl xai_tool_runtime::Tool for $tool {
            type Args = AskConsultInput;
            type Output = ToolOutput;

            fn id(&self) -> xai_tool_protocol::ToolId {
                xai_tool_protocol::ToolId::new($id).expect("valid tool id")
            }

            fn description(
                &self,
                _ctx: &::xai_tool_runtime::ListToolsContext,
            ) -> xai_tool_types::ToolDescription {
                xai_tool_types::ToolDescription::new(
                    $id,
                    ToolMetadata::sanitized_description_template(self),
                )
            }

            fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
                xai_tool_protocol::ToolCapabilities {
                    is_read_only: true,
                    tool_scope: Some(xai_tool_protocol::ToolScope::Read),
                    ..Default::default()
                }
            }

            async fn run(
                &self,
                _ctx: xai_tool_runtime::ToolCallContext,
                input: AskConsultInput,
            ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
                if session_is($provider) {
                    return Ok(ToolOutput::Text(
                        "This Ask tool is disabled because the session is already on that provider. Switch /model or use a different Ask* tool.".into(),
                    ));
                }
                let model = input
                    .model
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| $default_model.to_string());
                let effort = grog_providers::default_effort(&model);
                match grog_providers::consult::ask_with_effort(&model, &input.prompt, effort).await {
                    Ok(out) => Ok(ToolOutput::Text(out.text.into())),
                    Err(err) => Err(xai_tool_runtime::ToolError::execution(
                        xai_tool_protocol::ToolId::new($id).expect("valid"),
                        err.to_string(),
                    )),
                }
            }
        }
    };
}

ask_tool!(
    AskClaudeTool,
    "AskClaude",
    "Consult Claude Code (print-mode `claude`) for a second opinion. Auth is the user's Claude CLI, not grog. Disabled when /model is already a claude-bridge id.",
    grog_providers::ProviderId::ClaudeBridge,
    grog_providers::grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED
);
ask_tool!(
    AskAntigravityTool,
    "AskAntigravity",
    "Consult Google Antigravity (`agy -p`) for a second opinion. Auth is agy's Google login, not grog. Disabled when /model is already an antigravity id.",
    grog_providers::ProviderId::Antigravity,
    grog_providers::grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED
);
ask_tool!(
    AskCodexTool,
    "AskCodex",
    "Consult ChatGPT Codex via the user's Codex/ChatGPT subscription (not an OpenAI API key). Disabled when /model is already a codex id.",
    grog_providers::ProviderId::Codex,
    grog_providers::grog_codex::DEFAULT_CODEX_QUALIFIED
);
