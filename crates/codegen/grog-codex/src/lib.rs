//! ChatGPT Codex subscription provider for grog.
//!
//! Native OAuth (same protocol as the official Codex CLI). Not an OpenAI API
//! key. Prefer importing `~/.codex/auth.json` when the user already logged in
//! with `codex login`.

pub mod auth;
pub mod client;
pub mod models;
pub mod store;

pub use auth::{CodexAuth, CodexAuthError, auth_json_path, chatgpt_account_id, parse_auth_json};
pub use client::{
    ClientError, ConsultResult, ORIGINATOR, consult_body, consult_input, consult_sync,
    parse_consult_stream, refresh_sync,
};
pub use models::{
    CODEX_FALLBACK_MODELS, CONSUMER_CODEX_MODELS, CodexModel, DEFAULT_CODEX_EFFORT,
    DEFAULT_CODEX_MODEL, DEFAULT_CODEX_QUALIFIED, select_codex_model_id,
};
pub use store::{
    StoreError, grog_codex_auth_path, import_codex_cli_auth, load_auth, load_grog_or_import,
    save_auth,
};
