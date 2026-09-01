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

/// Claude Code `--model` id for Fable 5.1 (current Claude default).
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-fable-5-1";

/// Qualified `provider/model` form of [`DEFAULT_CLAUDE_MODEL`].
pub const DEFAULT_CLAUDE_QUALIFIED: &str = "claude-bridge/claude-fable-5-1";

/// Claude Code `--effort` for AskClaude / council (not a higher thinking tier).
/// Fable 5.1 defaults to High in Claude Code and Medium on Claude.ai; grog
/// keeps the prior Opus council default of medium.
pub const DEFAULT_CLAUDE_EFFORT: &str = "medium";

/// Picker order. First entry is [`DEFAULT_CLAUDE_MODEL`]. `fable` resolves
/// to Fable 5.1; `opus` still maps to the Opus 5 row; `sonnet` follows the
/// newest catalog id containing "sonnet".
pub const CLAUDE_BRIDGE_MODELS: &[ClaudeBridgeModel] = &[
    ClaudeBridgeModel {
        id: "claude-fable-5-1",
        display_name: "Fable 5.1",
    },
    ClaudeBridgeModel {
        id: "claude-fable-5",
        display_name: "Fable 5",
    },
    ClaudeBridgeModel {
        id: "claude-sonnet-5",
        display_name: "Sonnet 5",
    },
    ClaudeBridgeModel {
        id: "claude-opus-5",
        display_name: "Opus 5",
    },
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
pub struct CliModel<'a> {
    pub cli_id: &'a str,
    pub context_window: u32,
}

/// Map a grog picker id to the Claude Code `--model` string.
///
/// Unknown ids pass through as typed. Do **not** rewrite them to
/// `claude-sonnet-4-6` — that dropped `claude-fable-5-1` / `claude-sonnet-5`
/// before they had catalog rows.
pub fn resolve_cli_model<'a>(id: &'a str, settings: LongContextSettings) -> CliModel<'a> {
    let one_m_paid = settings.plan == Plan::Max || settings.extra_usage;
    match id {
        "claude-fable-5-1" => CliModel {
            cli_id: "claude-fable-5-1",
            context_window: CTX_1M,
        },
        "claude-fable-5" => CliModel {
            cli_id: "claude-fable-5",
            context_window: CTX_1M,
        },
        "claude-sonnet-5" => CliModel {
            cli_id: "claude-sonnet-5",
            context_window: CTX_200K,
        },
        "claude-opus-5" => CliModel {
            cli_id: "claude-opus-5",
            context_window: CTX_1M,
        },
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
        other => CliModel {
            cli_id: other,
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
    fn fable_51_is_current_claude_without_rewrite() {
        let m = resolve_cli_model("claude-fable-5-1", LongContextSettings::default());
        assert_eq!(m.cli_id, "claude-fable-5-1");
        assert_eq!(m.context_window, CTX_1M);
        assert_eq!(DEFAULT_CLAUDE_MODEL, "claude-fable-5-1");
        assert_eq!(DEFAULT_CLAUDE_QUALIFIED, "claude-bridge/claude-fable-5-1");
        assert_eq!(DEFAULT_CLAUDE_EFFORT, "medium");
        assert_eq!(CLAUDE_BRIDGE_MODELS[0].id, DEFAULT_CLAUDE_MODEL);
        assert!(
            CLAUDE_BRIDGE_MODELS
                .iter()
                .any(|m| m.id == "claude-fable-5"),
            "Fable 5 stays pickable"
        );
        assert!(
            CLAUDE_BRIDGE_MODELS
                .iter()
                .any(|m| m.id == "claude-sonnet-5"),
            "Sonnet 5 is the Claude Code sonnet alias"
        );
        assert!(
            !CLAUDE_BRIDGE_MODELS
                .iter()
                .any(|m| m.id.contains("mythos")),
            "Mythos is trusted-access only"
        );
    }

    #[test]
    fn opus_5_stays_in_catalog_without_1m_suffix() {
        let m = resolve_cli_model("claude-opus-5", LongContextSettings::default());
        assert_eq!(m.cli_id, "claude-opus-5");
        assert_eq!(m.context_window, CTX_1M);
        assert_ne!(DEFAULT_CLAUDE_MODEL, "claude-opus-5");
        assert!(CLAUDE_BRIDGE_MODELS.iter().any(|m| m.id == "claude-opus-5"));
    }

    #[test]
    fn sonnet_5_does_not_rewrite_to_sonnet_46() {
        let m = resolve_cli_model("claude-sonnet-5", LongContextSettings::default());
        assert_eq!(m.cli_id, "claude-sonnet-5");
        assert_ne!(m.cli_id, "claude-sonnet-4-6");
    }

    #[test]
    fn unknown_cli_id_passes_through_instead_of_rewriting_to_sonnet_46() {
        let m = resolve_cli_model("claude-fable-5-1", LongContextSettings::default());
        assert_eq!(m.cli_id, "claude-fable-5-1");
        let typed = resolve_cli_model("claude-not-in-catalog-yet", LongContextSettings::default());
        assert_eq!(typed.cli_id, "claude-not-in-catalog-yet");
        assert_ne!(typed.cli_id, "claude-sonnet-4-6");
    }

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
