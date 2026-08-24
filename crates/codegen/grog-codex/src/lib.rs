//! ChatGPT Codex subscription provider for grog.
//!
//! Native OAuth (same protocol as the official Codex CLI). Not an OpenAI API
//! key. Prefer importing `~/.codex/auth.json` when the user already logged in
//! with `codex login`.

pub mod auth;
pub mod models;

pub use auth::{auth_json_path, chatgpt_account_id, parse_auth_json, CodexAuth, CodexAuthError};
pub use models::{CodexModel, CODEX_FALLBACK_MODELS};
