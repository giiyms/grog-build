//! Grog provider registry: parse `provider/model` ids and list native catalogs.
//!
//! These crates reimplement the pi bridge packages in Rust. They are not a
//! host for the npm packages.

pub use grog_antigravity;
pub use grog_claude_bridge;
pub use grog_codex;

pub mod consult;
pub mod doctor;

use grog_antigravity::ANTIGRAVITY_FALLBACK_MODELS;
use grog_claude_bridge::CLAUDE_BRIDGE_MODELS;
use grog_codex::CODEX_FALLBACK_MODELS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderId {
    Http,
    Codex,
    ClaudeBridge,
    Antigravity,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Codex => "codex",
            Self::ClaudeBridge => "claude-bridge",
            Self::Antigravity => "antigravity",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "http" | "openai" | "anthropic" => Some(Self::Http),
            "codex" | "openai-codex" | "chatgpt" => Some(Self::Codex),
            "claude-bridge" | "claude" => Some(Self::ClaudeBridge),
            "antigravity" | "agy" | "gemini" => Some(Self::Antigravity),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    pub provider: ProviderId,
    pub model: String,
}

impl ModelRef {
    pub fn parse(s: &str) -> Self {
        if let Some((prov, model)) = s.split_once('/') {
            if let Some(provider) = ProviderId::parse(prov) {
                return Self {
                    provider,
                    model: model.to_string(),
                };
            }
        }
        // Bare ids: grok-* stay HTTP; claude-* go to the bridge.
        if s.starts_with("claude-") {
            return Self {
                provider: ProviderId::ClaudeBridge,
                model: s.to_string(),
            };
        }
        if s.starts_with("gemini-") {
            return Self {
                provider: ProviderId::Antigravity,
                model: s.to_string(),
            };
        }
        if s.contains("codex") {
            return Self {
                provider: ProviderId::Codex,
                model: s.to_string(),
            };
        }
        Self {
            provider: ProviderId::Http,
            model: s.to_string(),
        }
    }

    pub fn qualified(&self) -> String {
        format!("{}/{}", self.provider.as_str(), self.model)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    pub provider: ProviderId,
    pub id: &'static str,
    pub display_name: &'static str,
}

pub fn builtin_catalog() -> Vec<CatalogEntry> {
    let mut out = Vec::new();
    for m in CODEX_FALLBACK_MODELS {
        out.push(CatalogEntry {
            provider: ProviderId::Codex,
            id: m.id,
            display_name: m.display_name,
        });
    }
    for m in CLAUDE_BRIDGE_MODELS {
        out.push(CatalogEntry {
            provider: ProviderId::ClaudeBridge,
            id: m.id,
            display_name: m.display_name,
        });
    }
    for m in ANTIGRAVITY_FALLBACK_MODELS {
        out.push(CatalogEntry {
            provider: ProviderId::Antigravity,
            id: m.id,
            display_name: m.display_name,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qualified_ids() {
        let r = ModelRef::parse("claude-bridge/claude-opus-4-6");
        assert_eq!(r.provider, ProviderId::ClaudeBridge);
        assert_eq!(r.model, "claude-opus-4-6");
        let r = ModelRef::parse("agy/gemini-3.6-flash");
        assert_eq!(r.provider, ProviderId::Antigravity);
        let r = ModelRef::parse("codex/gpt-5.1-codex");
        assert_eq!(r.provider, ProviderId::Codex);
        assert_eq!(r.qualified(), grog_codex::DEFAULT_CODEX_QUALIFIED);
    }

    #[test]
    fn bare_claude_id_selects_bridge_not_anthropic_http() {
        let r = ModelRef::parse("claude-opus-4-8");
        assert_eq!(r.provider, ProviderId::ClaudeBridge);
        assert_eq!(r.qualified(), "claude-bridge/claude-opus-4-8");
    }

    #[test]
    fn catalog_includes_all_three_native_providers() {
        let cat = builtin_catalog();
        assert!(cat.iter().any(|e| e.provider == ProviderId::Codex));
        assert!(cat.iter().any(|e| e.provider == ProviderId::ClaudeBridge));
        assert!(cat.iter().any(|e| e.provider == ProviderId::Antigravity));
    }

    #[test]
    fn catalog_contains_council_member_ids() {
        let keys: Vec<String> = builtin_catalog()
            .iter()
            .map(|e| format!("{}/{}", e.provider.as_str(), e.id))
            .collect();
        assert!(keys.iter().any(|k| k == "claude-bridge/claude-opus-4-6"));
        assert!(keys.iter().any(|k| k == "antigravity/gemini-3.6-flash"));
        assert!(
            keys.iter()
                .any(|k| k == grog_codex::DEFAULT_CODEX_QUALIFIED)
        );
        assert_eq!(keys[0], grog_codex::DEFAULT_CODEX_QUALIFIED);
        assert_ne!(grog_codex::DEFAULT_CODEX_MODEL, "gpt-5.3-codex");
    }
}
