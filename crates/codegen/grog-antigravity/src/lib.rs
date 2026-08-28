//! Native Antigravity (`agy`) bridge for grog.
//!
//! Learned from `@estebanforge/pi-antigravity-bridge`, reimplemented in Rust:
//! spawn unmodified `agy`, poll its conversation sqlite, decode unpublished
//! protobuf step payloads. Grog never holds Google OAuth tokens.

pub mod consult;
pub mod models;
pub mod protobuf;
pub mod spawn;

pub use consult::{ConsultError, ConsultResult, ask_agy, provider_turn, run_print_plan};
pub use models::{ANTIGRAVITY_FALLBACK_MODELS, AntigravityModel, slugify_model};
pub use protobuf::{
    DecodedStep, Field, ProtobufError, ToolCall, extract_agent_text, extract_title,
    extract_tool_call, walk_fields,
};
pub use spawn::{
    AgyMode, AgySpawnPlan, AskAntigravitySpec, ProviderTurnSpec, ask_agy_argv, prompt_from_args,
    provider_turn_argv,
};
