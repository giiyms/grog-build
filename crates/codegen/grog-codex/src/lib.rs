//! ChatGPT Codex subscription provider for grog.
//!
//! Native OAuth (same protocol as the official Codex CLI). Not an OpenAI API
//! key. Prefer importing `~/.codex/auth.json` when the user already logged in
//! with `codex login`.

pub mod auth;
pub mod client;
pub mod models;
pub mod store;

pub use auth::{auth_json_path, chatgpt_account_id, parse_auth_json, CodexAuth, CodexAuthError};
pub use client::{consult_sync, refresh_sync, ClientError, ConsultResult};
pub use models::{CodexModel, CODEX_FALLBACK_MODELS};
pub use store::{
    grog_codex_auth_path, import_codex_cli_auth, load_auth, load_grog_or_import, save_auth,
    StoreError,
};
