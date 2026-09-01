//! Council-seat defaults, short-name resolution, complement picker, cycle.

use grog_providers::{ProviderId, consult, inference_route};

pub use grog_providers::ModelRef;

/// One advisor model assignment (qualified catalog id + thinking effort).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorSeat {
    pub qualified: String,
    pub short_name: String,
    pub display_name: String,
    /// Override; `None` means the provider's Ask*/council default.
    pub effort: Option<String>,
}

impl AdvisorSeat {
    pub fn effort_token(&self) -> Option<String> {
        self.effort
            .clone()
            .or_else(|| grog_providers::default_effort(&self.qualified).map(str::to_string))
    }

    pub fn provider(&self) -> ProviderId {
        ModelRef::parse(&self.qualified).provider
    }

    pub fn native_route_is_http(&self) -> bool {
        let parsed = ModelRef::parse(&self.qualified);
        let marker = format!("grog://{}", parsed.provider.as_str());
        inference_route(&self.qualified, &marker)
            .sampler_chat_completions_url(&marker)
            .is_some()
    }

    pub fn is_native(&self) -> bool {
        consult::is_native_model(&self.qualified)
    }
}

/// True for a trailing thinking/effort token (`/advisor luna xhigh`).
pub fn is_effort_token(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "none" | "low" | "medium" | "high" | "xhigh" | "max"
    )
}

pub fn luna() -> AdvisorSeat {
    AdvisorSeat {
        qualified: grog_providers::grog_codex::DEFAULT_CODEX_QUALIFIED.to_string(),
        short_name: "luna".into(),
        display_name: "GPT-5.6 Luna".into(),
        effort: None,
    }
}

pub fn fable() -> AdvisorSeat {
    AdvisorSeat {
        qualified: grog_providers::grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED.to_string(),
        short_name: "fable".into(),
        display_name: "Fable 5.1".into(),
        effort: None,
    }
}

pub fn opus() -> AdvisorSeat {
    AdvisorSeat {
        qualified: "claude-bridge/claude-opus-5".into(),
        short_name: "opus".into(),
        display_name: "Opus 5".into(),
        effort: None,
    }
}

pub fn agy() -> AdvisorSeat {
    AdvisorSeat {
        qualified: grog_providers::grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED.to_string(),
        short_name: "agy".into(),
        display_name: "Gemini 3.7 Flash High".into(),
        effort: None,
    }
}

/// Newest Sonnet actually listed in grog's Claude catalog. `None` if the
/// catalog has no sonnet id — callers must fail the alias rather than
/// silently mapping to Opus.
pub fn catalog_sonnet() -> Option<AdvisorSeat> {
    grog_providers::grog_claude_bridge::CLAUDE_BRIDGE_MODELS
        .iter()
        .filter(|m| m.id.to_ascii_lowercase().contains("sonnet"))
        .max_by_key(|m| version_key(m.id))
        .map(|m| AdvisorSeat {
            qualified: format!("claude-bridge/{}", m.id),
            short_name: "sonnet".into(),
            display_name: m.display_name.to_string(),
            effort: None,
        })
}

fn version_key(id: &str) -> Vec<u32> {
    id.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// Cycle order: distinct models Daniel cares about, not one-per-provider.
pub fn cycle_seats() -> Vec<AdvisorSeat> {
    let mut seats = vec![luna(), fable(), opus()];
    if let Some(sonnet) = catalog_sonnet() {
        seats.push(sonnet);
    }
    seats.push(agy());
    seats
}

/// Lowercase, strip punctuation to spaces, collapse whitespace, common typos.
pub fn normalize_alias(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_space = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    // "got 5.6 luna" / "got-5-6-luna" after punct strip is "got 5 6 luna".
    collapsed
        .replace("got 5 6", "gpt 5 6")
        .replace("got56", "gpt 5 6")
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error(
        "unknown advisor model '{0}'. Try luna, fable, opus, sonnet, claude, codex, agy, or a qualified id like codex/gpt-5.6-luna"
    )]
    Unknown(String),
    #[error(
        "no Sonnet model is in grog's Claude catalog; pick fable, opus, luna, or agy, or a qualified id"
    )]
    SonnetMissing,
}

/// Resolve a user token (short name or qualified id) to a seat.
pub fn resolve_short_name(raw: &str) -> Result<AdvisorSeat, ResolveError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ResolveError::Unknown(raw.to_string()));
    }
    if trimmed.contains('/') {
        return Ok(seat_from_qualified(trimmed));
    }
    let key = normalize_alias(trimmed);
    match key.as_str() {
        "luna" | "5 6" | "56" | "gpt 5 6" | "gpt 56" | "gpt 5 6 luna" | "gpt56luna"
        | "gpt 5 6 luna xhigh" => Ok(luna()),
        "fable" | "fable 5 1" | "fable51" | "claude fable" | "claude fable 5 1"
        | "claudefable51" => Ok(fable()),
        "opus" | "opus 5" | "opus5" | "claude opus 5" | "claudeopus5" => Ok(opus()),
        "sonnet" | "claude sonnet" => catalog_sonnet().ok_or(ResolveError::SonnetMissing),
        "sonnet 4 6" | "sonnet46" | "claude sonnet 4 6" => {
            Ok(seat_from_qualified("claude-bridge/claude-sonnet-4-6"))
        }
        "claude" => Ok(fable()),
        "codex" => Ok(luna()),
        "agy" | "gemini" | "flash" | "antigravity" => Ok(agy()),
        other => {
            // Bare catalog ids: claude-fable-5-1, gpt-5.6-luna, gemini-3.7-flash-high.
            let parsed = ModelRef::parse(trimmed);
            if parsed.provider != ProviderId::Http || trimmed.contains("grok") {
                return Ok(seat_from_qualified(&parsed.qualified()));
            }
            Err(ResolveError::Unknown(other.to_string()))
        }
    }
}

pub fn resolve_spec(raw: &str, effort: Option<&str>) -> Result<AdvisorSeat, ResolveError> {
    let mut seat = resolve_short_name(raw)?;
    if let Some(token) = effort.map(str::trim).filter(|s| !s.is_empty()) {
        if is_effort_token(token) {
            seat.effort = Some(token.to_ascii_lowercase());
        }
    }
    Ok(seat)
}

fn seat_from_qualified(id: &str) -> AdvisorSeat {
    let parsed = ModelRef::parse(id);
    let qualified = if parsed.provider == ProviderId::Http {
        parsed.model.clone()
    } else {
        parsed.qualified()
    };
    if let Some(known) = cycle_seats()
        .into_iter()
        .find(|s| s.qualified == qualified || s.qualified.ends_with(&format!("/{}", parsed.model)))
    {
        return known;
    }
    AdvisorSeat {
        display_name: display_name_for(&qualified),
        short_name: parsed.model.clone(),
        qualified,
        effort: None,
    }
}

/// Human label for status (`GPT-5.6 Luna`), falling back to the slug.
pub fn display_name_for(qualified: &str) -> String {
    let parsed = ModelRef::parse(qualified);
    match parsed.provider {
        ProviderId::Codex => grog_providers::grog_codex::CODEX_FALLBACK_MODELS
            .iter()
            .find(|m| m.id == parsed.model)
            .map(|m| m.display_name.to_string())
            .unwrap_or(parsed.model),
        ProviderId::ClaudeBridge => grog_providers::grog_claude_bridge::CLAUDE_BRIDGE_MODELS
            .iter()
            .find(|m| m.id == parsed.model)
            .map(|m| m.display_name.to_string())
            .unwrap_or(parsed.model),
        ProviderId::Antigravity => grog_providers::grog_antigravity::ANTIGRAVITY_FALLBACK_MODELS
            .iter()
            .find(|m| m.id == parsed.model)
            .map(|m| m.display_name.to_string())
            .unwrap_or(parsed.model),
        ProviderId::Http => parsed.model,
    }
}

/// Complement of the live primary so the advisor is never the same provider.
pub fn complement_seat(primary_model: &str) -> AdvisorSeat {
    let provider = ModelRef::parse(primary_model).provider;
    match provider {
        ProviderId::ClaudeBridge => luna(),
        ProviderId::Codex | ProviderId::Antigravity | ProviderId::Http => fable(),
    }
}

/// Walk luna → fable → opus → sonnet → agy, skipping seats on the primary's provider.
pub fn cycle_seat(current: Option<&AdvisorSeat>, primary_model: &str) -> AdvisorSeat {
    let seats = cycle_seats();
    let primary_p = ModelRef::parse(primary_model).provider;
    let walk: Vec<AdvisorSeat> = seats
        .iter()
        .filter(|s| s.provider() != primary_p)
        .cloned()
        .collect();
    let walk = if walk.is_empty() { seats } else { walk };
    let idx = current.and_then(|cur| {
        walk.iter()
            .position(|s| s.qualified == cur.qualified || s.short_name == cur.short_name)
    });
    match idx {
        Some(i) => walk[(i + 1) % walk.len()].clone(),
        None => walk[0].clone(),
    }
}

/// Doctor-style readiness for status (`claude not on PATH`, missing Codex tokens).
pub fn seat_readiness(seat: &AdvisorSeat) -> Option<String> {
    let want = seat.provider();
    grog_providers::doctor::doctor_checks()
        .into_iter()
        .find(|c| c.provider == want)
        .and_then(|c| if c.ok { None } else { Some(c.detail) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luna_aliases_including_typos() {
        for raw in [
            "luna",
            "Luna",
            "5.6",
            "gpt-5.6",
            "gpt-5.6-luna",
            "gpt 5.6 luna",
            "got 5.6 luna",
            "GOT-5.6-LUNA",
        ] {
            let seat = resolve_short_name(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            assert_eq!(seat.qualified, "codex/gpt-5.6-luna", "raw={raw}");
            assert_eq!(seat.short_name, "luna");
            assert_eq!(seat.display_name, "GPT-5.6 Luna");
            assert_eq!(seat.effort_token().as_deref(), Some("xhigh"));
        }
    }

    #[test]
    fn fable_aliases_resolve_to_fable_51() {
        for raw in [
            "fable",
            "Fable",
            "fable-5.1",
            "fable 5.1",
            "claude-fable-5-1",
            "claude-bridge/claude-fable-5-1",
        ] {
            let seat = resolve_short_name(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            assert_eq!(
                seat.qualified, "claude-bridge/claude-fable-5-1",
                "raw={raw}"
            );
            assert_eq!(seat.short_name, "fable", "raw={raw}");
            assert_eq!(seat.display_name, "Fable 5.1", "raw={raw}");
            assert_eq!(seat.effort_token().as_deref(), Some("medium"));
        }
    }

    #[test]
    fn opus_aliases() {
        for raw in ["opus", "opus 5", "opus-5", "claude-opus-5", "OPUS"] {
            let seat = resolve_short_name(raw).expect(raw);
            assert_eq!(seat.qualified, "claude-bridge/claude-opus-5");
            assert_eq!(seat.short_name, "opus");
            assert_eq!(seat.display_name, "Opus 5");
            assert_eq!(seat.effort_token().as_deref(), Some("medium"));
            assert_ne!(seat.qualified, fable().qualified);
        }
    }

    #[test]
    fn sonnet_resolves_to_catalog_slug_not_opus() {
        let seat = resolve_short_name("sonnet").expect("sonnet must exist in this tree");
        assert_eq!(seat.qualified, "claude-bridge/claude-sonnet-5");
        assert_eq!(seat.short_name, "sonnet");
        assert_eq!(seat.display_name, "Sonnet 5");
        assert_ne!(seat.qualified, opus().qualified);
        assert_ne!(seat.qualified, fable().qualified);
        assert!(
            grog_providers::grog_claude_bridge::CLAUDE_BRIDGE_MODELS
                .iter()
                .any(|m| m.id == "claude-sonnet-5"),
            "test pins the newest sonnet slug actually in grog-claude-bridge"
        );
        let older = resolve_short_name("sonnet-4.6").expect("versioned 4.6 alias");
        assert_eq!(older.qualified, "claude-bridge/claude-sonnet-4-6");
    }

    #[test]
    fn family_aliases() {
        assert_eq!(resolve_short_name("claude").unwrap().short_name, "fable");
        assert_eq!(
            resolve_short_name("claude").unwrap().qualified,
            grog_providers::grog_claude_bridge::DEFAULT_CLAUDE_QUALIFIED
        );
        assert_eq!(resolve_short_name("codex").unwrap().short_name, "luna");
        assert_eq!(resolve_short_name("agy").unwrap().short_name, "agy");
        assert_eq!(resolve_short_name("gemini").unwrap().short_name, "agy");
        assert_eq!(resolve_short_name("flash").unwrap().short_name, "agy");
        assert_eq!(
            resolve_short_name("agy").unwrap().qualified,
            grog_providers::grog_antigravity::DEFAULT_ANTIGRAVITY_QUALIFIED
        );
    }

    #[test]
    fn qualified_ids_pass_through() {
        let seat = resolve_short_name("codex/gpt-5.6-luna").unwrap();
        assert_eq!(seat.short_name, "luna");
        let seat = resolve_short_name("claude-bridge/claude-opus-5").unwrap();
        assert_eq!(seat.short_name, "opus");
        let seat = resolve_short_name("claude-bridge/claude-fable-5-1").unwrap();
        assert_eq!(seat.short_name, "fable");
    }

    #[test]
    fn trailing_effort_overrides_default() {
        let seat = resolve_spec("luna", Some("medium")).unwrap();
        assert_eq!(seat.effort_token().as_deref(), Some("medium"));
        let seat = resolve_spec("opus", Some("high")).unwrap();
        assert_eq!(seat.effort_token().as_deref(), Some("high"));
        let seat = resolve_spec("sonnet", Some("high")).unwrap();
        assert_eq!(seat.effort_token().as_deref(), Some("high"));
    }

    #[test]
    fn complement_never_matches_primary_provider() {
        assert_eq!(complement_seat("grok-4").short_name, "fable");
        assert_eq!(
            complement_seat("grok-4").provider(),
            ProviderId::ClaudeBridge
        );
        assert_eq!(complement_seat("codex/gpt-5.6-luna").short_name, "fable");
        assert_eq!(
            complement_seat("claude-bridge/claude-opus-5").short_name,
            "luna"
        );
        assert_eq!(
            complement_seat("claude-bridge/claude-fable-5-1").short_name,
            "luna"
        );
        assert_eq!(
            complement_seat("claude-bridge/claude-sonnet-4-6").short_name,
            "luna"
        );
        assert_eq!(
            complement_seat("antigravity/gemini-3.7-flash-high").short_name,
            "fable"
        );
        for primary in [
            "grok-4",
            "codex/gpt-5.6-luna",
            "claude-bridge/claude-fable-5-1",
            "claude-bridge/claude-opus-5",
            "antigravity/gemini-3.7-flash-high",
        ] {
            assert_ne!(
                complement_seat(primary).provider(),
                ModelRef::parse(primary).provider,
                "primary={primary}"
            );
        }
    }

    #[test]
    fn cycle_skips_primary_provider() {
        let next = cycle_seat(Some(&luna()), "grok-4");
        assert_eq!(next.short_name, "fable");
        let next = cycle_seat(Some(&fable()), "grok-4");
        assert_eq!(next.short_name, "opus");
        let next = cycle_seat(Some(&opus()), "grok-4");
        assert_eq!(next.short_name, "sonnet");
        let next = cycle_seat(Some(&catalog_sonnet().unwrap()), "grok-4");
        assert_eq!(next.short_name, "agy");
        let next = cycle_seat(Some(&agy()), "grok-4");
        assert_eq!(next.short_name, "luna");

        // Primary is Claude: skip fable, opus, and sonnet.
        let next = cycle_seat(Some(&luna()), "claude-bridge/claude-opus-5");
        assert_eq!(next.short_name, "agy");
        assert_ne!(next.provider(), ProviderId::ClaudeBridge);
        let next = cycle_seat(Some(&luna()), "claude-bridge/claude-fable-5-1");
        assert_eq!(next.short_name, "agy");
        let next = cycle_seat(Some(&agy()), "claude-opus-5");
        assert_eq!(next.short_name, "luna");

        // Primary is Codex: skip luna.
        let next = cycle_seat(Some(&opus()), "codex/gpt-5.6-luna");
        assert_ne!(next.provider(), ProviderId::Codex);
    }

    #[test]
    fn council_seats_never_post_grog_scheme_through_http() {
        for seat in cycle_seats() {
            assert!(
                seat.is_native(),
                "{} must use grog-providers::consult",
                seat.qualified
            );
            assert!(
                !seat.native_route_is_http(),
                "{} must not yield grog://…/chat/completions",
                seat.qualified
            );
        }
    }

    #[test]
    fn unknown_alias_errors() {
        let err = resolve_short_name("not-a-model").unwrap_err();
        assert!(matches!(err, ResolveError::Unknown(_)));
    }
}
