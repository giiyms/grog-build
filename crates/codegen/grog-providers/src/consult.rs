//! Dispatch a consult to a native grog provider. HTTP models stay on the sampler.

use std::cell::Cell;
use std::path::PathBuf;

use crate::{ModelRef, ProviderId};

tokio::task_local! {
    static IN_CONSULT: Cell<bool>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultOutcome {
    pub text: String,
    pub session_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConsultError {
    #[error("HTTP sampler models are not dispatched through grog-providers")]
    HttpPassthrough,
    #[error("nested Ask*/council consults are not allowed")]
    Nested,
    #[error("prompt is empty")]
    EmptyPrompt,
    #[error(transparent)]
    Claude(#[from] grog_claude_bridge::ConsultError),
    #[error(transparent)]
    Antigravity(#[from] grog_antigravity::ConsultError),
    #[error(transparent)]
    Codex(#[from] grog_codex::ClientError),
    #[error(transparent)]
    CodexStore(#[from] grog_codex::StoreError),
    #[error("{0}")]
    Other(String),
}

pub fn is_native_model(model_id: &str) -> bool {
    ModelRef::parse(model_id).provider != ProviderId::Http
}

fn in_consult() -> bool {
    IN_CONSULT.try_with(|flag| flag.get()).unwrap_or(false)
}

/// Isolated Ask* consult (`read` / skip-permissions / Codex Responses).
pub async fn ask(model_id: &str, prompt: &str) -> Result<ConsultOutcome, ConsultError> {
    ask_with_effort(model_id, prompt, crate::default_effort(model_id)).await
}

/// Ask* / council consult with an explicit thinking-effort token.
pub async fn ask_with_effort(
    model_id: &str,
    prompt: &str,
    effort: Option<&str>,
) -> Result<ConsultOutcome, ConsultError> {
    dispatch(model_id, prompt, ConsultKind::Ask, effort).await
}

/// Full-session provider turn for `/model` on a native backend.
pub async fn provider_turn(model_id: &str, prompt: &str) -> Result<ConsultOutcome, ConsultError> {
    provider_turn_with_effort(model_id, prompt, crate::default_effort(model_id)).await
}

pub async fn provider_turn_with_effort(
    model_id: &str,
    prompt: &str,
    effort: Option<&str>,
) -> Result<ConsultOutcome, ConsultError> {
    dispatch(model_id, prompt, ConsultKind::ProviderTurn, effort).await
}

#[derive(Clone, Copy)]
enum ConsultKind {
    Ask,
    ProviderTurn,
}

async fn dispatch(
    model_id: &str,
    prompt: &str,
    kind: ConsultKind,
    effort: Option<&str>,
) -> Result<ConsultOutcome, ConsultError> {
    if prompt.trim().is_empty() {
        return Err(ConsultError::EmptyPrompt);
    }
    let parsed = ModelRef::parse(model_id);
    if parsed.provider == ProviderId::Http {
        return Err(ConsultError::HttpPassthrough);
    }
    if in_consult() {
        return Err(ConsultError::Nested);
    }
    IN_CONSULT
        .scope(Cell::new(true), async move {
            match (parsed.provider, kind) {
                (ProviderId::ClaudeBridge, ConsultKind::Ask) => {
                    let out = grog_claude_bridge::ask_claude(prompt, &parsed.model, effort).await?;
                    Ok(ConsultOutcome {
                        text: out.text,
                        session_id: out.session_id,
                    })
                }
                (ProviderId::ClaudeBridge, ConsultKind::ProviderTurn) => {
                    let out =
                        grog_claude_bridge::provider_turn(prompt, &parsed.model, effort).await?;
                    Ok(ConsultOutcome {
                        text: out.text,
                        session_id: out.session_id,
                    })
                }
                (ProviderId::Antigravity, ConsultKind::Ask) => {
                    let out = grog_antigravity::ask_agy(prompt, &parsed.model, effort).await?;
                    Ok(ConsultOutcome {
                        text: out.text,
                        session_id: None,
                    })
                }
                (ProviderId::Antigravity, ConsultKind::ProviderTurn) => {
                    let out =
                        grog_antigravity::provider_turn(prompt, &parsed.model, effort).await?;
                    Ok(ConsultOutcome {
                        text: out.text,
                        session_id: None,
                    })
                }
                (ProviderId::Codex, _) => consult_codex(&parsed.model, prompt, effort).await,
                (ProviderId::Http, _) => Err(ConsultError::HttpPassthrough),
            }
        })
        .await
}

async fn consult_codex(
    model: &str,
    prompt: &str,
    effort: Option<&str>,
) -> Result<ConsultOutcome, ConsultError> {
    let grog_home = grog_home_dir();
    let user_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let model = model.to_string();
    let prompt = prompt.to_string();
    let effort = effort.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        let effort = effort.as_deref();
        let mut auth = grog_codex::load_grog_or_import(&grog_home, &user_home)?;
        match grog_codex::consult_sync(&auth, &model, &prompt, effort) {
            Ok(out) => Ok(ConsultOutcome {
                text: out.text,
                session_id: None,
            }),
            Err(grog_codex::ClientError::Http { status: 401, .. }) => {
                auth = grog_codex::refresh_sync(&auth)?;
                let _ = grog_codex::save_auth(&grog_codex::grog_codex_auth_path(&grog_home), &auth);
                let out = grog_codex::consult_sync(&auth, &model, &prompt, effort)?;
                Ok(ConsultOutcome {
                    text: out.text,
                    session_id: None,
                })
            }
            Err(err) => Err(ConsultError::from(err)),
        }
    })
    .await
    .map_err(|e| ConsultError::Other(e.to_string()))?
}

fn grog_home_dir() -> PathBuf {
    xai_grok_home::grok_home()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn http_models_are_not_dispatched() {
        let err = ask("grok-4", "hello").await.unwrap_err();
        assert!(matches!(err, ConsultError::HttpPassthrough));
        assert!(!is_native_model("grok-4"));
        assert!(is_native_model("claude-bridge/claude-opus-5"));
        assert!(is_native_model("claude-opus-5"));
        assert!(is_native_model("claude-bridge/claude-opus-4-6"));
        assert!(is_native_model("claude-opus-4-6"));
    }

    #[tokio::test]
    async fn nested_consult_is_denied() {
        IN_CONSULT
            .scope(Cell::new(true), async {
                let err = ask("claude-opus-5", "hello").await.unwrap_err();
                assert!(matches!(err, ConsultError::Nested));
            })
            .await;
    }

    #[tokio::test]
    async fn empty_prompt_is_rejected() {
        let err = provider_turn(crate::grog_codex::DEFAULT_CODEX_QUALIFIED, "  ")
            .await
            .unwrap_err();
        assert!(matches!(err, ConsultError::EmptyPrompt));
    }
}
