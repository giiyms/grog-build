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

/// Catalog marker for native grog providers. This is **not** an HTTP origin.
/// Reqwest will refuse to POST it (`grog://codex/chat/completions`). Turns
/// must go through [`consult`] / the sampler intercept, never the HTTP client.
pub const NATIVE_URL_SCHEME: &str = "grog://";

/// `true` for the `grog://codex` (etc.) marker URLs merged into the catalog.
pub fn is_native_base_url(url: &str) -> bool {
    url.starts_with(NATIVE_URL_SCHEME)
}

/// Reconstruct a consult id (`codex/gpt-5.6-luna`) from a catalog key, a
/// slug, and/or a `grog://` marker URL.
///
/// Council children historically stored only the slug (`gpt-5.6-luna`) on
/// `SamplerConfig.model` while `base_url` was `grog://codex`. The slug
/// alone looks like HTTP (`is_native_model` is false), so the intercept
/// must also read the marker URL.
pub fn consult_model_id(model_id: &str, base_url: Option<&str>) -> Option<String> {
    if consult::is_native_model(model_id) {
        return Some(model_id.to_string());
    }
    let url = base_url?;
    let rest = url.strip_prefix(NATIVE_URL_SCHEME)?;
    let provider = rest.split('/').next().filter(|s| !s.is_empty())?;
    let pid = ProviderId::parse(provider)?;
    if pid == ProviderId::Http {
        return None;
    }
    if model_id.is_empty() {
        return None;
    }
    Some(format!("{}/{}", pid.as_str(), model_id))
}

/// Where a `(model id, base_url)` pair should send tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceRoute {
    /// Claude / Codex / Antigravity — `grog-providers::consult`, not reqwest.
    Native {
        provider: ProviderId,
        qualified: String,
    },
    Http,
}

impl InferenceRoute {
    /// URL the HTTP sampler would POST. `None` for native routes so reqwest
    /// never sees `grog://codex/chat/completions`.
    pub fn sampler_chat_completions_url(&self, base_url: &str) -> Option<String> {
        match self {
            Self::Native { .. } => None,
            Self::Http => {
                if is_native_base_url(base_url) {
                    return None;
                }
                Some(format!(
                    "{}/chat/completions",
                    base_url.trim_end_matches('/')
                ))
            }
        }
    }

    pub fn is_native(&self) -> bool {
        matches!(self, Self::Native { .. })
    }
}

/// HTTP URL title-gen would POST via reqwest. Native seats return `None` so
/// we never build `grog://codex/chat/completions` (live council title-gen
/// error: `builder error for url (grog://…/chat/completions)`).
pub fn title_generation_reqwest_url(model_id: &str, base_url: &str) -> Option<String> {
    inference_route(model_id, base_url).sampler_chat_completions_url(base_url)
}

/// `true` when session title generation must not call the HTTP sampler.
/// Council children and `/model` native seats skip the LLM title and use
/// truncated user text instead of POSTing `grog://` through reqwest.
pub fn skip_http_title_generation(model_id: &str, base_url: Option<&str>) -> bool {
    consult_model_id(model_id, base_url).is_some() || base_url.is_some_and(is_native_base_url)
}

/// Sampler `model` when the aux Grok title model cannot be resolved.
/// Native primaries keep a qualified consult id so [`skip_http_title_generation`]
/// fires, instead of stamping `grok-4.6` onto a `grog://codex` client.
pub fn title_generation_fallback_model(
    requested_summary_model: &str,
    primary_model: &str,
    primary_base_url: &str,
) -> String {
    if skip_http_title_generation(primary_model, Some(primary_base_url)) {
        consult_model_id(primary_model, Some(primary_base_url))
            .unwrap_or_else(|| primary_model.to_string())
    } else {
        requested_summary_model.to_string()
    }
}

pub fn inference_route(model_id: &str, base_url: &str) -> InferenceRoute {
    match consult_model_id(model_id, Some(base_url)) {
        Some(qualified) => {
            let parsed = ModelRef::parse(&qualified);
            InferenceRoute::Native {
                provider: parsed.provider,
                qualified,
            }
        }
        None => InferenceRoute::Http,
    }
}

/// Default thinking-effort token for Ask*/council when the caller does not
/// set one. agy max is `high`; Codex Luna uses `xhigh`; Claude Opus 5 uses
/// `medium`.
pub fn default_effort(model_id: &str) -> Option<&'static str> {
    match ModelRef::parse(model_id).provider {
        ProviderId::Codex => Some(grog_codex::DEFAULT_CODEX_EFFORT),
        ProviderId::ClaudeBridge => Some(grog_claude_bridge::DEFAULT_CLAUDE_EFFORT),
        ProviderId::Antigravity => Some(grog_antigravity::DEFAULT_ANTIGRAVITY_EFFORT),
        ProviderId::Http => None,
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
        let r = ModelRef::parse("claude-bridge/claude-opus-5");
        assert_eq!(r.provider, ProviderId::ClaudeBridge);
        assert_eq!(r.model, "claude-opus-5");
        let r = ModelRef::parse("agy/gemini-3.7-flash-high");
        assert_eq!(r.provider, ProviderId::Antigravity);
        let r = ModelRef::parse("codex/gpt-5.6-luna");
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
    fn default_effort_matches_council_seats() {
        assert_eq!(
            default_effort(grog_codex::DEFAULT_CODEX_QUALIFIED),
            Some("xhigh")
        );
        assert_eq!(
            default_effort(grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED),
            Some("medium")
        );
        assert_eq!(
            default_effort(grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED),
            Some("high")
        );
        assert_eq!(default_effort("grok-4"), None);
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
        assert!(
            keys.iter()
                .any(|k| k == grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED)
        );
        assert!(keys.iter().any(|k| k == "claude-bridge/claude-opus-4-6"));
        assert!(
            keys.iter()
                .any(|k| k == grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED)
        );
        assert!(
            keys.iter()
                .any(|k| k == "antigravity/gemini-3.7-flash-medium")
        );
        assert!(keys.iter().any(|k| k == "antigravity/gemini-3.6-flash"));
        assert!(
            keys.iter()
                .any(|k| k == grog_codex::DEFAULT_CODEX_QUALIFIED)
        );
        assert_eq!(keys[0], grog_codex::DEFAULT_CODEX_QUALIFIED);
        assert_eq!(grog_codex::DEFAULT_CODEX_MODEL, "gpt-5.6-luna");
        assert_ne!(grog_codex::DEFAULT_CODEX_MODEL, "gpt-5.3-codex");
        assert_ne!(grog_codex::DEFAULT_CODEX_MODEL, "gpt-5.1-codex");
    }

    #[test]
    fn luna_slug_alone_is_http_until_the_grog_marker_url_is_present() {
        assert!(!consult::is_native_model("gpt-5.6-luna"));
        assert_eq!(
            consult_model_id("gpt-5.6-luna", None),
            None,
            "bare Luna is not Codex; that is why council children died"
        );
        assert_eq!(
            consult_model_id("gpt-5.6-luna", Some("grog://codex")),
            Some(grog_codex::DEFAULT_CODEX_QUALIFIED.to_string())
        );
        assert_eq!(
            consult_model_id(grog_codex::DEFAULT_CODEX_QUALIFIED, Some("grog://codex")),
            Some(grog_codex::DEFAULT_CODEX_QUALIFIED.to_string())
        );
    }

    #[test]
    fn council_seats_never_produce_a_grog_scheme_reqwest_url() {
        for (model, base) in [
            (grog_codex::DEFAULT_CODEX_QUALIFIED, "grog://codex"),
            ("gpt-5.6-luna", "grog://codex"),
            (
                grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED,
                "grog://claude-bridge",
            ),
            ("claude-opus-5", "grog://claude-bridge"),
            (
                grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED,
                "grog://antigravity",
            ),
            ("gemini-3.7-flash-high", "grog://antigravity"),
        ] {
            let route = inference_route(model, base);
            assert!(
                route.is_native(),
                "{model} + {base} must be a native consult, not HTTP"
            );
            assert_eq!(
                route.sampler_chat_completions_url(base),
                None,
                "{model} must not yield grog://…/chat/completions"
            );
        }
        let http = inference_route("grok-4", "https://api.x.ai/v1");
        assert_eq!(http, InferenceRoute::Http);
        assert_eq!(
            http.sampler_chat_completions_url("https://api.x.ai/v1"),
            Some("https://api.x.ai/v1/chat/completions".into())
        );
    }

    #[test]
    fn native_marker_url_is_not_http() {
        assert!(is_native_base_url("grog://codex"));
        assert!(is_native_base_url("grog://claude-bridge"));
        assert!(is_native_base_url("grog://antigravity"));
        assert!(!is_native_base_url("https://api.x.ai/v1"));
        assert!(!is_native_base_url("https://chatgpt.com/backend-api"));
    }

    #[test]
    fn title_generation_never_builds_grog_scheme_reqwest_url() {
        for (model, base) in [
            (grog_codex::DEFAULT_CODEX_QUALIFIED, "grog://codex"),
            ("gpt-5.6-luna", "grog://codex"),
            (
                grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED,
                "grog://claude-bridge",
            ),
            ("claude-opus-5", "grog://claude-bridge"),
            (
                grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED,
                "grog://antigravity",
            ),
            ("gemini-3.7-flash-high", "grog://antigravity"),
        ] {
            assert_eq!(
                title_generation_reqwest_url(model, base),
                None,
                "{model} title-gen must not yield grog://…/chat/completions for reqwest"
            );
            assert!(
                skip_http_title_generation(model, Some(base)),
                "{model} + {base} skips HTTP title-gen"
            );
        }
        assert!(skip_http_title_generation(
            grog_codex::DEFAULT_CODEX_QUALIFIED,
            None
        ));
        assert!(skip_http_title_generation("claude-opus-5", None));
        assert!(skip_http_title_generation(
            "antigravity/gemini-3.7-flash-high",
            None
        ));
        assert!(!skip_http_title_generation(
            "grok-4.6",
            Some("https://api.x.ai/v1")
        ));
        assert_eq!(
            title_generation_reqwest_url("grok-4.6", "https://api.x.ai/v1"),
            Some("https://api.x.ai/v1/chat/completions".into())
        );
        assert_eq!(
            title_generation_fallback_model("grok-4.6", "codex/gpt-5.6-luna", "grog://codex"),
            grog_codex::DEFAULT_CODEX_QUALIFIED
        );
        assert_eq!(
            title_generation_fallback_model("grok-4.6", "grok-4.6", "https://api.x.ai/v1"),
            "grok-4.6"
        );
    }

    #[test]
    fn codex_consult_body_input_is_a_list() {
        let body = grog_codex::consult_body(
            grog_codex::DEFAULT_CODEX_MODEL,
            "What is 2+2? Reply with one sentence.",
            grog_codex::DEFAULT_CODEX_EFFORT,
        );
        assert!(
            body["input"].is_array(),
            "live Codex 400: Input must be a list"
        );
        assert_eq!(body["input"][0]["type"], "message");
        assert_eq!(grog_codex::ORIGINATOR, "grog");
    }
}
