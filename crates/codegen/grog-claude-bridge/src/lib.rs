//! Native Claude Code bridge for grog.
//!
//! Learned from `pi-claude-bridge`, reimplemented in Rust:
//! spawn the user's `claude` CLI (print + stream-json), never hold an
//! Anthropic subscription inside grog, and expose AskClaude as a consult tool.

pub mod consult;
pub mod models;
pub mod spawn;
pub mod stream;

pub use models::{
    resolve_cli_model, AskClaudeMode, ClaudeBridgeModel, LongContextSettings, Plan,
    CLAUDE_BRIDGE_MODELS,
};
pub use spawn::{
    ask_claude_argv, provider_turn_argv, AskClaudeSpec, ClaudeSpawnPlan, ProviderTurnSpec,
};
pub use consult::{ask_claude, provider_turn, run_print_plan, ConsultError, ConsultResult};
pub use stream::{parse_stream_json_line, StreamEvent};
