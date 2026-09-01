//! Resolve a persisted advisor seat from `models.advisor` (config.toml).
//!
//! Enable/disable is never persisted — only the chosen model (and optional
//! effort) live in `[models]`, parallel to `models.default`.

use grog_providers::ModelRef;

use crate::seats::{AdvisorSeat, complement_seat, resolve_spec};

/// Build a seat from config strings. `None` if unset or unresolvable.
pub fn seat_from_config(advisor: Option<&str>, effort: Option<&str>) -> Option<AdvisorSeat> {
    let raw = advisor.map(str::trim).filter(|s| !s.is_empty())?;
    resolve_spec(raw, effort).ok()
}

/// Prefer a persisted `models.advisor` seat when it is a different provider
/// than the live primary; otherwise the complement of the primary.
pub fn prefer_config_or_complement(
    config_advisor: Option<&str>,
    config_effort: Option<&str>,
    primary_model: &str,
) -> (AdvisorSeat, SeatSource) {
    if let Some(seat) = seat_from_config(config_advisor, config_effort) {
        let primary_p = ModelRef::parse(primary_model).provider;
        if seat.provider() != primary_p {
            return (seat, SeatSource::Config);
        }
    }
    (complement_seat(primary_model), SeatSource::Complement)
}

/// Where the live advisor seat came from. Shown by `/advisor status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeatSource {
    #[default]
    None,
    /// Loaded from `[models].advisor`.
    Config,
    /// `/advisor luna` (or picker commit) this session.
    SessionOverride,
    /// Complement of the live primary; no `models.advisor` (or it matched primary).
    Complement,
}

impl SeatSource {
    pub fn status_label(self) -> &'static str {
        match self {
            Self::None => "unset",
            Self::Config => "models.advisor",
            Self::SessionOverride => "session override",
            Self::Complement => "complement of primary",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip_does_not_encode_enabled() {
        let seat = seat_from_config(Some("codex/gpt-5.6-luna"), Some("xhigh")).unwrap();
        assert_eq!(seat.qualified, "codex/gpt-5.6-luna");
        assert_eq!(seat.short_name, "luna");
        assert!(seat_from_config(None, None).is_none());
        assert!(seat_from_config(Some(""), None).is_none());
        assert!(seat_from_config(Some("   "), None).is_none());
    }

    #[test]
    fn prefer_config_when_different_provider() {
        let (seat, src) = prefer_config_or_complement(
            Some("codex/gpt-5.6-luna"),
            None,
            "grok-4",
        );
        assert_eq!(seat.short_name, "luna");
        assert_eq!(src, SeatSource::Config);
    }

    #[test]
    fn prefer_complement_when_config_matches_primary_provider() {
        let (seat, src) = prefer_config_or_complement(
            Some("codex/gpt-5.6-luna"),
            None,
            "codex/gpt-5.6-luna",
        );
        assert_eq!(seat.short_name, "fable");
        assert_eq!(src, SeatSource::Complement);
    }

    #[test]
    fn prefer_complement_when_unset() {
        let (seat, src) = prefer_config_or_complement(None, None, "grok-4");
        assert_eq!(seat.short_name, "fable");
        assert_eq!(src, SeatSource::Complement);
    }
}
