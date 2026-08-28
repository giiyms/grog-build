//! Native Claude Code bridge for grog.
//!
//! Learned from `pi-claude-bridge`, reimplemented in Rust:
//! spawn the user's `claude` CLI (print + stream-json), never hold an
//! Anthropic subscription inside grog, and expose AskClaude as a consult tool.

pub mod consult;
pub mod models;
pub mod spawn;
pub mod stream;

pub use consult::{ConsultError, ConsultResult, ask_claude, provider_turn, run_print_plan};
pub use models::{
    AskClaudeMode, CLAUDE_BRIDGE_MODELS, ClaudeBridgeModel, DEFAULT_CLAUDE_EFFORT,
    DEFAULT_CLAUDE_MODEL, DEFAULT_CLAUDE_QUALIFIED, LongContextSettings, Plan, resolve_cli_model,
};
pub use spawn::{
    AskClaudeSpec, ClaudeSpawnPlan, ProviderTurnSpec, ask_claude_argv, provider_turn_argv,
};
pub use stream::{StreamEvent, parse_stream_json_line};
