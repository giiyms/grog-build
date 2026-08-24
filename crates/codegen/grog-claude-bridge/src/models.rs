//! Model catalog and Claude Code `[1m]` suffix policy.
//!
//! Context windows are measured Agent SDK / Claude Code behavior, not the
//! public Anthropic API card. See `pi-claude-bridge` `src/models.ts`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Pro,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongContextSettings {
    pub plan: Plan,
    /// Pay Extra Usage for 1M on models that are not 1M by default.
    pub extra_usage: bool,
}

impl Default for LongContextSettings {
    fn default() -> Self {
        Self {
            plan: Plan::Pro,
            extra_usage: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaudeBridgeModel {
    pub id: &'static str,
    pub display_name: &'static str,
}

/// Picker order. `opus` resolves to the first opus entry (`claude-opus-4-6`).
pub const CLAUDE_BRIDGE_MODELS: &[ClaudeBridgeModel] = &[
    ClaudeBridgeModel {
        id: "claude-opus-4-6",
        display_name: "Opus 4.6",
    },
    ClaudeBridgeModel {
        id: "claude-sonnet-4-6",
        display_name: "Sonnet 4.6",
    },
    ClaudeBridgeModel {
        id: "claude-haiku-4-5",
        display_name: "Haiku 4.5",
    },
    ClaudeBridgeModel {
        id: "claude-opus-4-7",
        display_name: "Opus 4.7",
    },
    ClaudeBridgeModel {
        id: "claude-opus-4-8",
        display_name: "Opus 4.8",
    },
];

const CTX_200K: u32 = 200_000;
const CTX_1M: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliModel {
    pub cli_id: &'static str,
    pub context_window: u32,
}

/// Map a grog picker id to the Claude Code `--model` string.
pub fn resolve_cli_model(id: &str, settings: LongContextSettings) -> CliModel {
    let one_m_paid = settings.plan == Plan::Max || settings.extra_usage;
    match id {
        "claude-opus-4-7" => CliModel {
            cli_id: "claude-opus-4-7",
            context_window: CTX_1M,
        },
        "claude-opus-4-8" => CliModel {
            cli_id: "claude-opus-4-8[1m]",
            context_window: CTX_1M,
        },
        "claude-opus-4-6" => {
            if one_m_paid {
                CliModel {
                    cli_id: "claude-opus-4-6[1m]",
                    context_window: CTX_1M,
                }
            } else {
                CliModel {
                    cli_id: "claude-opus-4-6",
                    context_window: CTX_200K,
                }
            }
        }
        "claude-sonnet-4-6" => {
            if settings.extra_usage {
                CliModel {
                    cli_id: "claude-sonnet-4-6[1m]",
                    context_window: CTX_1M,
                }
            } else {
                CliModel {
                    cli_id: "claude-sonnet-4-6",
                    context_window: CTX_200K,
                }
            }
        }
        "claude-haiku-4-5" => CliModel {
            cli_id: "claude-haiku-4-5",
            context_window: CTX_200K,
        },
        _ => CliModel {
            cli_id: "claude-sonnet-4-6",
            context_window: CTX_200K,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskClaudeMode {
    /// Read files / search; no writes (default).
    Read,
    /// No tools.
    None,
    /// Read + write + bash. Opt-in.
    Full,
}

impl AskClaudeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::None => "none",
            Self::Full => "full",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_47_is_1m_without_suffix() {
        let m = resolve_cli_model("claude-opus-4-7", LongContextSettings::default());
        assert_eq!(m.cli_id, "claude-opus-4-7");
        assert_eq!(m.context_window, CTX_1M);
    }

    #[test]
    fn opus_48_always_requests_1m_suffix() {
        let m = resolve_cli_model("claude-opus-4-8", LongContextSettings::default());
        assert_eq!(m.cli_id, "claude-opus-4-8[1m]");
        assert_eq!(m.context_window, CTX_1M);
    }

    #[test]
    fn opus_46_1m_requires_max_or_extra_usage() {
        let pro = resolve_cli_model("claude-opus-4-6", LongContextSettings::default());
        assert_eq!(pro.cli_id, "claude-opus-4-6");
        assert_eq!(pro.context_window, CTX_200K);

        let max = resolve_cli_model(
            "claude-opus-4-6",
            LongContextSettings {
                plan: Plan::Max,
                extra_usage: false,
            },
        );
        assert_eq!(max.cli_id, "claude-opus-4-6[1m]");
        assert_eq!(max.context_window, CTX_1M);
    }

    #[test]
    fn sonnet_1m_only_with_extra_usage() {
        let max_only = resolve_cli_model(
            "claude-sonnet-4-6",
            LongContextSettings {
                plan: Plan::Max,
                extra_usage: false,
            },
        );
        assert_eq!(max_only.cli_id, "claude-sonnet-4-6");

        let extra = resolve_cli_model(
            "claude-sonnet-4-6",
            LongContextSettings {
                plan: Plan::Pro,
                extra_usage: true,
            },
        );
        assert_eq!(extra.cli_id, "claude-sonnet-4-6[1m]");
    }
}
