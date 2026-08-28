//! Fallback Codex model ids until live catalog fetch exists.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexModel {
    pub id: &'static str,
    pub display_name: &'static str,
}

pub const CODEX_FALLBACK_MODELS: &[CodexModel] = &[
    CodexModel {
        id: "gpt-5.3-codex",
        display_name: "GPT-5.3 Codex",
    },
    CodexModel {
        id: "gpt-5.2-codex",
        display_name: "GPT-5.2 Codex",
    },
    CodexModel {
        id: "o4-mini",
        display_name: "o4-mini",
    },
];
