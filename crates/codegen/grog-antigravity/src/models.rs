//! Fallback catalog when `agy models` is unavailable.
//!
//! `agy models` / `agy --model` use dotted catalog slugs such as
//! `gemini-3.6-flash-high`. Thinking is baked into Flash slugs
//! (`-low` / `-medium` / `-high`) and is also exposed as `--effort
//! low|medium|high` — there is no xhigh/max. The grog default is Gemini 3.7
//! Flash at that max: `gemini-3.7-flash-high` plus `--effort high`.
//!
//! Do not pass pi-style slugify ids (`gemini-3-7-flash-high`) to `--model`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntigravityModel {
    pub id: &'static str,
    pub display_name: &'static str,
}

/// agy `--model` slug for Gemini 3.7 Flash at max thinking (`-high`).
/// Same dotted catalog form as the existing `gemini-3.6-flash-high` entry.
pub const DEFAULT_ANTIGRAVITY_MODEL: &str = "gemini-3.7-flash-high";

/// Qualified `provider/model` form of [`DEFAULT_ANTIGRAVITY_MODEL`].
pub const DEFAULT_ANTIGRAVITY_QUALIFIED: &str = "antigravity/gemini-3.7-flash-high";

/// Max thinking agy actually supports (`--effort high`; not xhigh/max).
pub const DEFAULT_ANTIGRAVITY_EFFORT: &str = "high";

pub const ANTIGRAVITY_FALLBACK_MODELS: &[AntigravityModel] = &[
    AntigravityModel {
        id: "gemini-3.7-flash-high",
        display_name: "Gemini 3.7 Flash High",
    },
    AntigravityModel {
        id: "gemini-3.7-flash-medium",
        display_name: "Gemini 3.7 Flash Medium",
    },
    AntigravityModel {
        id: "gemini-3.7-flash-low",
        display_name: "Gemini 3.7 Flash Low",
    },
    AntigravityModel {
        id: "gemini-3.6-flash",
        display_name: "Gemini 3.6 Flash",
    },
    AntigravityModel {
        id: "gemini-3.6-flash-low",
        display_name: "Gemini 3.6 Flash Low",
    },
    AntigravityModel {
        id: "gemini-3.6-flash-high",
        display_name: "Gemini 3.6 Flash High",
    },
    AntigravityModel {
        id: "gemini-3.1-pro",
        display_name: "Gemini 3.1 Pro",
    },
];

/// Map a thinking token onto agy's `--effort` values (`low`/`medium`/`high`).
/// `xhigh`/`max` clamp to `high` — that is the highest agy accepts.
pub fn agy_effort_flag(effort: Option<&str>) -> &'static str {
    match effort {
        Some("low") => "low",
        Some("medium") => "medium",
        Some("high") | Some("xhigh") | Some("max") | None => DEFAULT_ANTIGRAVITY_EFFORT,
        _ => DEFAULT_ANTIGRAVITY_EFFORT,
    }
}

pub fn slugify_model(display: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true;
    for ch in display.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_gemini_37_flash_high() {
        assert_eq!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3.7-flash-high");
        assert_eq!(
            DEFAULT_ANTIGRAVITY_QUALIFIED,
            "antigravity/gemini-3.7-flash-high"
        );
        assert_eq!(DEFAULT_ANTIGRAVITY_EFFORT, "high");
        assert_eq!(ANTIGRAVITY_FALLBACK_MODELS[0].id, DEFAULT_ANTIGRAVITY_MODEL);
        assert!(
            ANTIGRAVITY_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gemini-3.7-flash-medium")
        );
        assert!(
            ANTIGRAVITY_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gemini-3.6-flash")
        );
        assert!(
            ANTIGRAVITY_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gemini-3.6-flash-high")
        );
        assert_ne!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3-7-flash-high");
        assert_ne!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3.6-flash");
        assert_ne!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3.7-flash");
    }

    #[test]
    fn agy_max_effort_is_high_not_xhigh() {
        assert_eq!(agy_effort_flag(None), "high");
        assert_eq!(agy_effort_flag(Some("high")), "high");
        assert_eq!(agy_effort_flag(Some("xhigh")), "high");
        assert_eq!(agy_effort_flag(Some("max")), "high");
        assert_eq!(agy_effort_flag(Some("medium")), "medium");
        assert_eq!(agy_effort_flag(Some("low")), "low");
    }

    #[test]
    fn slugifies_agy_display_names() {
        assert_eq!(
            slugify_model("Gemini 3.6 Flash (Medium)"),
            "gemini-3-6-flash-medium"
        );
        assert_eq!(
            slugify_model("Gemini 3.7 Flash (High)"),
            "gemini-3-7-flash-high"
        );
        assert_eq!(slugify_model("Gemini 3.1 Pro"), "gemini-3-1-pro");
    }
}
