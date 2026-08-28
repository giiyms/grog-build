//! Fallback Codex model ids until live catalog fetch exists.
//!
//! ChatGPT Plus/Pro Codex subscriptions advertise a consumer set that does
//! **not** include `gpt-5.3-codex` (that id is rejected on consumer ChatGPT
//! accounts). Council, AskCodex, and the documented grog default use
//! [`DEFAULT_CODEX_MODEL`]. Prefer [`select_codex_model_id`] when a live
//! advertised list is available after login.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexModel {
    pub id: &'static str,
    pub display_name: &'static str,
}

/// Default Codex id for council / AskCodex / `[models].default`.
/// Consumer ChatGPT Codex subscriptions can run this; they reject `gpt-5.3-codex`.
pub const DEFAULT_CODEX_MODEL: &str = "gpt-5.1-codex";

/// Qualified `provider/model` form of [`DEFAULT_CODEX_MODEL`].
pub const DEFAULT_CODEX_QUALIFIED: &str = "codex/gpt-5.1-codex";

/// Consumer-first advertised ids. `gpt-5.3-codex` is kept in the fallback
/// catalog for API-org accounts but is never the unadvertised default.
pub const CONSUMER_CODEX_MODELS: &[&str] = &[
    "gpt-5.1-codex",
    "gpt-5.1-codex-max",
    "gpt-5.1-codex-mini",
    "gpt-5-codex",
    "codex-mini-latest",
    "o4-mini",
];

pub const CODEX_FALLBACK_MODELS: &[CodexModel] = &[
    CodexModel {
        id: "gpt-5.1-codex",
        display_name: "GPT-5.1 Codex",
    },
    CodexModel {
        id: "gpt-5.1-codex-mini",
        display_name: "GPT-5.1 Codex Mini",
    },
    CodexModel {
        id: "codex-mini-latest",
        display_name: "Codex Mini",
    },
    CodexModel {
        id: "gpt-5.2-codex",
        display_name: "GPT-5.2 Codex",
    },
    CodexModel {
        id: "gpt-5.3-codex",
        display_name: "GPT-5.3 Codex",
    },
    CodexModel {
        id: "o4-mini",
        display_name: "o4-mini",
    },
];

/// Pick a Codex catalog id the subscription actually listed.
///
/// Prefers the consumer set so ChatGPT Plus/Pro accounts do not start on
/// `gpt-5.3-codex`. If the advertised list is empty, returns
/// [`DEFAULT_CODEX_MODEL`]. Org accounts that only advertise `gpt-5.3-codex`
/// still get that id (it was listed).
pub fn select_codex_model_id(advertised: &[impl AsRef<str>]) -> String {
    if advertised.is_empty() {
        return DEFAULT_CODEX_MODEL.to_string();
    }
    for preferred in CONSUMER_CODEX_MODELS {
        if advertised.iter().any(|id| id.as_ref() == *preferred) {
            return (*preferred).to_string();
        }
    }
    advertised[0].as_ref().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_consumer_not_gpt_5_3() {
        assert_eq!(DEFAULT_CODEX_MODEL, "gpt-5.1-codex");
        assert_eq!(DEFAULT_CODEX_QUALIFIED, "codex/gpt-5.1-codex");
        assert_eq!(CODEX_FALLBACK_MODELS[0].id, DEFAULT_CODEX_MODEL);
        assert_ne!(DEFAULT_CODEX_MODEL, "gpt-5.3-codex");
        assert!(
            CODEX_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gpt-5.3-codex"),
            "org accounts can still pick gpt-5.3-codex from the catalog"
        );
    }

    #[test]
    fn select_prefers_advertised_consumer_id() {
        assert_eq!(select_codex_model_id(&[] as &[&str]), "gpt-5.1-codex");
        assert_eq!(
            select_codex_model_id(&["gpt-5.3-codex", "gpt-5.1-codex"]),
            "gpt-5.1-codex"
        );
        assert_eq!(
            select_codex_model_id(&["codex-mini-latest"]),
            "codex-mini-latest"
        );
        assert_eq!(
            select_codex_model_id(&["gpt-5.3-codex"]),
            "gpt-5.3-codex",
            "an org that only advertises gpt-5.3-codex may use it"
        );
    }
}
