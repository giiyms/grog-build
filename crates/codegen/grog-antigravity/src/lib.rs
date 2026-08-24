//! Native Antigravity (`agy`) bridge for grog.
//!
//! Learned from `@estebanforge/pi-antigravity-bridge`, reimplemented in Rust:
//! spawn unmodified `agy`, poll its conversation sqlite, decode unpublished
//! protobuf step payloads. Grog never holds Google OAuth tokens.

pub mod models;
pub mod protobuf;
pub mod spawn;

pub use models::{slugify_model, AntigravityModel, ANTIGRAVITY_FALLBACK_MODELS};
pub use protobuf::{
    extract_agent_text, extract_title, extract_tool_call, walk_fields, DecodedStep,
};
pub use spawn::{
    ask_agy_argv, provider_turn_argv, AgyMode, AgySpawnPlan, AskAntigravitySpec, ProviderTurnSpec,
};
