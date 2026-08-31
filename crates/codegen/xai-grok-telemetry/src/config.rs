//! These types live here so the data-collector engine can construct a [`TelemetryClient`](crate::client::TelemetryClient) without depending on shell.
//!
//! Shell still re-exports these types from their original paths so existing call sites (and `Config` derive impls) compile unchanged.
use serde::{Deserialize, Serialize};
/// Telemetry mode: `true`/`false` (legacy bool) or `"session_metrics"` (string).
///
/// - `Disabled`: nothing sent (enterprise default)
/// - `SessionMetrics`: metadata-only lifecycle events, no content
/// - `Enabled`: full product telemetry (events and Mixpanel)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TelemetryMode {
    #[default]
    Disabled,
    SessionMetrics,
    Enabled,
}
impl TelemetryMode {
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }
    /// True for both `SessionMetrics` and `Enabled`.
    pub fn session_metrics_enabled(&self) -> bool {
        matches!(self, Self::SessionMetrics | Self::Enabled)
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "enabled" | "full" => Some(Self::Enabled),
            "0" | "false" | "no" | "off" | "disabled" => Some(Self::Disabled),
            "session-metrics" | "session_metrics" => Some(Self::SessionMetrics),
            _ => None,
        }
    }
}
#[cfg(test)]
mod telemetry_mode_tests {
    use super::TelemetryMode;
    /// A parent process hands its resolved mode to spawned children via `GROK_TELEMETRY_ENABLED={mode}` (Display).
    /// Every Display output must parse back to the same mode.
    #[test]
    fn display_round_trips_through_parse() {
        for mode in [
            TelemetryMode::Enabled,
            TelemetryMode::Disabled,
            TelemetryMode::SessionMetrics,
        ] {
            assert_eq!(
                TelemetryMode::parse(&mode.to_string()),
                Some(mode),
                "Display value for {mode:?} must parse back to itself"
            );
        }
    }
}
impl std::fmt::Display for TelemetryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => write!(f, "false"),
            Self::SessionMetrics => write!(f, "session_metrics"),
            Self::Enabled => write!(f, "true"),
        }
    }
}
impl From<bool> for TelemetryMode {
    fn from(b: bool) -> Self {
        if b { Self::Enabled } else { Self::Disabled }
    }
}
impl serde::Serialize for TelemetryMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Disabled => serializer.serialize_bool(false),
            Self::Enabled => serializer.serialize_bool(true),
            Self::SessionMetrics => serializer.serialize_str("session_metrics"),
        }
    }
}
/// Wire format for `[features] telemetry`: accepts `true`, `false`, or `"session_metrics"`.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum TelemetryModeValue {
    Bool(bool),
    Str(String),
}
impl<'de> serde::Deserialize<'de> for TelemetryMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match TelemetryModeValue::deserialize(deserializer)? {
            TelemetryModeValue::Bool(b) => Ok(Self::from(b)),
            TelemetryModeValue::Str(s) => Ok(Self::parse(&s).unwrap_or_else(|| {
                tracing::warn!(
                    value = %s,
                    "TELEMETRY_MODE_UNKNOWN: unrecognized telemetry mode; treating as disabled",
                );
                Self::Disabled
            })),
        }
    }
}
/// Parse an env var as a `TelemetryMode`. Returns `None` if unset or empty.
pub fn env_telemetry_mode(name: &str) -> Option<TelemetryMode> {
    let value = std::env::var(name).ok()?;
    TelemetryMode::parse(&value)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Declared for `serde_ignored`. Actual toggle is `[features] telemetry`.
    #[serde(default)]
    pub enabled: Option<bool>,
    pub events_url: Option<String>,
    pub events_api_key: Option<String>,
    pub mixpanel_token: Option<String>,
    pub mixpanel_enabled: bool,
    /// `None` inherits from `[features] telemetry`; `Some(false)` disables GCS uploads only.
    pub trace_upload: Option<bool>,
    /// External OTEL master switch (env `GROK_EXTERNAL_OTEL` wins).
    pub otel_enabled: Option<bool>,
    /// External OTEL metrics exporter: `otlp` | `console` | `none`.
    pub otel_metrics_exporter: Option<String>,
    /// External OTEL logs/events exporter: `otlp` | `console` | `none`.
    pub otel_logs_exporter: Option<String>,
    /// External OTLP base endpoint (`/v1/logs`, `/v1/metrics` appended for HTTP).
    pub otel_endpoint: Option<String>,
    /// External OTLP transport: `http/protobuf` | `grpc`.
    #[serde(alias = "otel_transport")]
    pub otel_protocol: Option<String>,
    pub otel_certificate: Option<String>,
    pub otel_client_certificate: Option<String>,
    pub otel_client_key: Option<String>,
    /// External OTEL content gate (admins can pin to `false` via requirements).
    pub otel_log_user_prompts: Option<bool>,
    /// External OTEL content gate (admins can pin to `false` via requirements).
    pub otel_log_tool_details: Option<bool>,
}
fn internal_defaults() -> (Option<String>, Option<String>, Option<String>, bool) {
    (None, None, None, false)
}
impl Default for TelemetryConfig {
    fn default() -> Self {
        // Grog never ships xAI Mixpanel / events URLs, even if a downstream
        // build sets `GROK_TELEMETRY_BUILD_*`. User config and runtime env
        // remain the only opt-in.
        let (events_url, events_api_key, mixpanel_token, mixpanel_enabled) = internal_defaults();
        Self {
            enabled: None,
            events_url,
            events_api_key,
            mixpanel_token,
            mixpanel_enabled,
            trace_upload: None,
            otel_enabled: None,
            otel_metrics_exporter: None,
            otel_logs_exporter: None,
            otel_endpoint: None,
            otel_protocol: None,
            otel_certificate: None,
            otel_client_certificate: None,
            otel_client_key: None,
            otel_log_user_prompts: None,
            otel_log_tool_details: None,
        }
    }
}
/// Prefer `GROG_TELEMETRY_ENABLED` over `GROK_TELEMETRY_ENABLED`.
pub fn env_telemetry_mode_grog_or_grok() -> Option<TelemetryMode> {
    env_telemetry_mode("GROG_TELEMETRY_ENABLED")
        .or_else(|| env_telemetry_mode("GROK_TELEMETRY_ENABLED"))
}

impl TelemetryConfig {
    pub fn apply_env_overrides(&mut self) {
        self.normalize();
        if let Some(value) = Self::env_override_grog_or_grok("GROK_TELEMETRY_EVENTS_URL") {
            self.events_url = value;
        }
        if let Some(value) = Self::env_override_grog_or_grok("GROK_TELEMETRY_EVENTS_API_KEY") {
            self.events_api_key = value;
        }
        if let Some(value) = Self::env_override_grog_or_grok("GROK_TELEMETRY_MIXPANEL_TOKEN") {
            self.mixpanel_token = value;
        }
        if let Some(value) =
            xai_grok_config::env_bool_grog_or_grok("GROK_TELEMETRY_MIXPANEL_ENABLED")
        {
            self.mixpanel_enabled = value;
        }
        if let Some(value) = xai_grok_config::env_bool_grog_or_grok("GROK_TELEMETRY_TRACE_UPLOAD") {
            self.trace_upload = Some(value);
        }
    }
    fn normalize(&mut self) {
        self.events_url = Self::normalize_optional_string(self.events_url.take());
        self.events_api_key = Self::normalize_optional_string(self.events_api_key.take());
        self.mixpanel_token = Self::normalize_optional_string(self.mixpanel_token.take());
    }
    fn env_override_grog_or_grok(grok_name: &str) -> Option<Option<String>> {
        if let Some(grog) = xai_grok_config::grog_env_alias(grok_name)
            && let Some(value) = Self::env_override(&grog)
        {
            return Some(value);
        }
        Self::env_override(grok_name)
    }

    fn env_override(name: &str) -> Option<Option<String>> {
        match std::env::var(name) {
            Ok(value) => Some(Self::normalize_optional_string(Some(value))),
            Err(_) => None,
        }
    }
    fn normalize_optional_string(value: Option<String>) -> Option<String> {
        value.and_then(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
    }
}
/// Derive a stable deployment ID (UUIDv5) from the deployment key.
pub fn deployment_id_from_key(key: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, key.as_bytes()).to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_ships_no_xai_telemetry_endpoints() {
        let cfg = TelemetryConfig::default();
        assert!(!cfg.mixpanel_enabled);
        assert_eq!(cfg.events_url, None);
        assert_eq!(cfg.events_api_key, None);
        assert_eq!(cfg.mixpanel_token, None);
        assert_eq!(cfg.trace_upload, None);
        assert_eq!(cfg.otel_enabled, None);
    }
}
