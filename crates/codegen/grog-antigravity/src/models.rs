//! `agy models` / `agy --model` use dotted catalog slugs such as
//! `gemini-3.8-flash-high`. Thinking is baked into Flash slugs
//! (`-low` / `-medium` / `-high`) and is also exposed as `--effort
//! low|medium|high` — there is no xhigh/max. The grog default is Gemini 3.8
//! Flash at that max: `gemini-3.8-flash-high` plus `--effort high`.
//!
//! Do not pass pi-style slugify ids (`gemini-3-8-flash-high`) to `--model`.

use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntigravityModel {
    pub id: &'static str,
    pub display_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedAntigravityModel {
    pub id: String,
    pub display_name: String,
}

/// agy `--model` slug for Gemini 3.8 Flash at max thinking (`-high`).
/// Same dotted catalog form as the existing `gemini-3.6-flash-high` entry.
pub const DEFAULT_ANTIGRAVITY_MODEL: &str = "gemini-3.8-flash-high";

/// Qualified `provider/model` form of [`DEFAULT_ANTIGRAVITY_MODEL`].
pub const DEFAULT_ANTIGRAVITY_QUALIFIED: &str = "antigravity/gemini-3.8-flash-high";

/// Max thinking agy actually supports (`--effort high`; not xhigh/max).
pub const DEFAULT_ANTIGRAVITY_EFFORT: &str = "high";

pub const ANTIGRAVITY_FALLBACK_MODELS: &[AntigravityModel] = &[
    AntigravityModel {
        id: "gemini-3.8-flash-high",
        display_name: "Gemini 3.8 Flash High",
    },
    AntigravityModel {
        id: "gemini-3.8-flash-medium",
        display_name: "Gemini 3.8 Flash Medium",
    },
    AntigravityModel {
        id: "gemini-3.8-flash-low",
        display_name: "Gemini 3.8 Flash Low",
    },
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
    AntigravityModel {
        id: "gemini-3.1-pro-high",
        display_name: "Gemini 3.1 Pro High",
    },
    AntigravityModel {
        id: "gemini-3.1-pro-low",
        display_name: "Gemini 3.1 Pro Low",
    },
];

const AGY_MODELS_TIMEOUT: Duration = Duration::from_secs(2);

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

fn fallback_listed() -> Vec<ListedAntigravityModel> {
    ANTIGRAVITY_FALLBACK_MODELS
        .iter()
        .map(|m| ListedAntigravityModel {
            id: m.id.to_string(),
            display_name: m.display_name.to_string(),
        })
        .collect()
}

fn looks_like_agy_id(id: &str) -> bool {
    !id.is_empty()
        && id.contains('-')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

/// Parse `agy models` stdout. First column is the catalog slug (not slugified).
pub fn parse_agy_models_output(stdout: &str) -> Vec<ListedAntigravityModel> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if first.eq_ignore_ascii_case("id") || first.eq_ignore_ascii_case("model") {
            continue;
        }
        let (id, name) = if let Some((id, rest)) = line.split_once('\t') {
            (id.trim(), rest.trim())
        } else if let Some(idx) = line.find("  ") {
            (line[..idx].trim(), line[idx..].trim())
        } else {
            let mut bits = line.split_whitespace();
            match (bits.next(), bits.next()) {
                (Some(id), None) => (id, id),
                (Some(id), Some(_)) if looks_like_agy_id(id) => (id, line[id.len()..].trim()),
                _ => continue,
            }
        };
        if !looks_like_agy_id(id) {
            continue;
        }
        let display_name = if name.is_empty() {
            id.to_string()
        } else {
            name.to_string()
        };
        if out.iter().any(|m: &ListedAntigravityModel| m.id == id) {
            continue;
        }
        out.push(ListedAntigravityModel {
            id: id.to_string(),
            display_name,
        });
    }
    out
}

fn merge_live_with_fallback(live: &[ListedAntigravityModel]) -> Vec<ListedAntigravityModel> {
    let mut out = live.to_vec();
    for fallback in fallback_listed() {
        if !out.iter().any(|m| m.id == fallback.id) {
            out.push(fallback);
        }
    }
    out
}

fn run_agy_models() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("grog-agy-models".into())
        .spawn(move || {
            let output = Command::new("agy")
                .arg("models")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output();
            let _ = tx.send(output);
        })
        .ok()?;
    let output = rx.recv_timeout(AGY_MODELS_TIMEOUT).ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

pub fn try_discover_agy_models() -> Option<Vec<ListedAntigravityModel>> {
    let stdout = run_agy_models()?;
    let parsed = parse_agy_models_output(&stdout);
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

pub fn listed_models() -> Vec<ListedAntigravityModel> {
    static CACHE: OnceLock<Vec<ListedAntigravityModel>> = OnceLock::new();
    CACHE
        .get_or_init(|| match try_discover_agy_models() {
            Some(live) => merge_live_with_fallback(&live),
            None => fallback_listed(),
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_gemini_38_flash_high() {
        assert_eq!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3.8-flash-high");
        assert_eq!(
            DEFAULT_ANTIGRAVITY_QUALIFIED,
            "antigravity/gemini-3.8-flash-high"
        );
        assert_eq!(DEFAULT_ANTIGRAVITY_EFFORT, "high");
        assert_eq!(ANTIGRAVITY_FALLBACK_MODELS[0].id, DEFAULT_ANTIGRAVITY_MODEL);
        assert!(
            ANTIGRAVITY_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gemini-3.8-flash-medium")
        );
        assert!(
            ANTIGRAVITY_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gemini-3.8-flash-low")
        );
        assert!(
            ANTIGRAVITY_FALLBACK_MODELS
                .iter()
                .any(|m| m.id == "gemini-3.7-flash-high")
        );
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
        assert_ne!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3-8-flash-high");
        assert_ne!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3.7-flash-high");
        assert_ne!(DEFAULT_ANTIGRAVITY_MODEL, "gemini-3.8-flash");
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
        assert_eq!(
            slugify_model("Gemini 3.8 Flash (High)"),
            "gemini-3-8-flash-high"
        );
        assert_eq!(slugify_model("Gemini 3.1 Pro"), "gemini-3-1-pro");
    }

    #[test]
    fn parse_agy_models_keeps_dotted_catalog_slugs() {
        let stdout = "\
gemini-3.8-flash-high\tGemini 3.8 Flash (High)
gemini-3.8-flash-medium\tGemini 3.8 Flash (Medium)
gemini-3.8-flash-low\tGemini 3.8 Flash (Low)
gemini-3.7-flash-high\tGemini 3.7 Flash (High)
gemini-3.1-pro-high  Gemini 3.1 Pro (High)
";
        let parsed = parse_agy_models_output(stdout);
        assert_eq!(parsed[0].id, "gemini-3.8-flash-high");
        assert_eq!(parsed[0].display_name, "Gemini 3.8 Flash (High)");
        assert!(parsed.iter().any(|m| m.id == "gemini-3.8-flash-medium"));
        assert!(parsed.iter().any(|m| m.id == "gemini-3.8-flash-low"));
        assert!(parsed.iter().any(|m| m.id == "gemini-3.1-pro-high"));
        assert!(
            parsed.iter().all(|m| !m.id.contains("gemini-3-8")),
            "must not slugify live agy ids"
        );
    }

    #[test]
    fn merge_live_keeps_fallback_ids_live_missed() {
        let live = vec![ListedAntigravityModel {
            id: "gemini-3.8-flash-high".into(),
            display_name: "Gemini 3.8 Flash (High)".into(),
        }];
        let merged = merge_live_with_fallback(&live);
        assert_eq!(merged[0].id, "gemini-3.8-flash-high");
        assert!(merged.iter().any(|m| m.id == "gemini-3.7-flash-high"));
        assert!(merged.iter().any(|m| m.id == "gemini-3.6-flash"));
    }
}
