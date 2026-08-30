//! Advisor note types and parsing of the sidecar's compact reply.

use std::fmt;

/// How strongly to weigh one accepted note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Nit = 1,
    Concern = 2,
    Blocker = 3,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nit => "nit",
            Self::Concern => "concern",
            Self::Blocker => "blocker",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "nit" | "nits" | "aside" => Some(Self::Nit),
            "concern" | "concerns" => Some(Self::Concern),
            "blocker" | "blockers" | "block" => Some(Self::Blocker),
            _ => None,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How to inject an accepted note into the primary session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Non-interrupting aside; fold at the next step boundary.
    Aside,
    /// Steer into the live turn (or a mid-work yield).
    Steer,
    /// Visible card only — do not wake a completed terminal turn.
    Card,
    /// Wake a follow-up even after a terminal "done".
    FollowUp,
}

/// A parsed note from the advisor model (not yet accepted by the guard).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorNote {
    pub severity: Severity,
    pub text: String,
}

/// A note the emission guard accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedNote {
    pub severity: Severity,
    pub text: String,
    pub advisor_short: String,
}

impl AcceptedNote {
    /// Primary-transcript line. Advisor notes are recognized by this prefix
    /// so the next delta can strip them (no recursive self-review).
    pub fn render_line(&self) -> String {
        format!(
            "[advisor {}] {}: {}",
            self.advisor_short, self.severity, self.text
        )
    }

    pub fn delivery(self_severity: Severity, turn_running: bool, terminal_answer: bool) -> Delivery {
        match self_severity {
            Severity::Nit => {
                if turn_running {
                    Delivery::Aside
                } else {
                    Delivery::Card
                }
            }
            Severity::Concern => {
                if turn_running {
                    Delivery::Steer
                } else if terminal_answer {
                    Delivery::Card
                } else {
                    Delivery::Steer
                }
            }
            Severity::Blocker => {
                if turn_running {
                    Delivery::Steer
                } else {
                    Delivery::FollowUp
                }
            }
        }
    }
}

/// Parse the sidecar's compact reply. `SILENCE` / empty / LGTM → `None`.
pub fn parse_advisor_output(raw: &str) -> Option<AdvisorNote> {
    let text = strip_fences(raw).trim().to_string();
    if text.is_empty() || is_silence(&text) {
        return None;
    }
    if let Some(note) = parse_json(&text) {
        return validate(note);
    }
    if let Some(note) = parse_labeled(&text) {
        return validate(note);
    }
    // Bare sentence: treat as a nit unless it starts with a severity word.
    let note = AdvisorNote {
        severity: Severity::Nit,
        text: collapse_ws(&text),
    };
    validate(note)
}

fn validate(note: AdvisorNote) -> Option<AdvisorNote> {
    let text = collapse_ws(&note.text);
    if text.is_empty() || is_silence(&text) {
        return None;
    }
    Some(AdvisorNote {
        severity: note.severity,
        text,
    })
}

fn is_silence(text: &str) -> bool {
    let n = crate::guard::normalize_key(text);
    matches!(
        n.as_str(),
        "silence"
            | "silent"
            | "none"
            | "n a"
            | "na"
            | "ok"
            | "okay"
            | "lgtm"
            | "looks good"
            | "looks good to me"
            | "no issue"
            | "no issues"
            | "no issue continue"
            | "nothing to add"
            | "nothing"
            | "continue"
            | "stop"
            | "done"
            | "complete"
            | "all good"
            | "no note"
    ) || n.starts_with("silence")
}

fn strip_fences(raw: &str) -> &str {
    let t = raw.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    t.strip_suffix("```").unwrap_or(t)
}

fn parse_json(text: &str) -> Option<AdvisorNote> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let v: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    let note = v.get("note").or_else(|| v.get("text"))?.as_str()?.trim();
    if note.is_empty() {
        return None;
    }
    let severity = v
        .get("severity")
        .and_then(|s| s.as_str())
        .and_then(Severity::parse)
        .unwrap_or(Severity::Nit);
    Some(AdvisorNote {
        severity,
        text: note.to_string(),
    })
}

fn parse_labeled(text: &str) -> Option<AdvisorNote> {
    let mut severity = None;
    let mut note = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.split_once(':') {
            let key = rest.0.trim().to_ascii_lowercase();
            let val = rest.1.trim();
            if key == "severity" || key == "level" {
                severity = Severity::parse(val);
            } else if key == "note" || key == "advice" || key == "text" {
                note = Some(val.to_string());
            } else if let Some(sev) = Severity::parse(&key) {
                severity = Some(sev);
                if !val.is_empty() {
                    note = Some(val.to_string());
                }
            }
        }
    }
    let text = note?;
    Some(AdvisorNote {
        severity: severity.unwrap_or(Severity::Nit),
        text,
    })
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_default() {
        assert!(parse_advisor_output("SILENCE").is_none());
        assert!(parse_advisor_output("  silence  ").is_none());
        assert!(parse_advisor_output("LGTM").is_none());
        assert!(parse_advisor_output("nothing to add").is_none());
        assert!(parse_advisor_output("").is_none());
    }

    #[test]
    fn json_and_labeled() {
        let n = parse_advisor_output(r#"{"severity":"concern","note":"tests never ran"}"#).unwrap();
        assert_eq!(n.severity, Severity::Concern);
        assert_eq!(n.text, "tests never ran");
        let n = parse_advisor_output("severity: blocker\nnote: the build is red").unwrap();
        assert_eq!(n.severity, Severity::Blocker);
        let n = parse_advisor_output("nit: unused import in main.rs").unwrap();
        assert_eq!(n.severity, Severity::Nit);
    }

    #[test]
    fn delivery_matrix() {
        assert_eq!(
            AcceptedNote::delivery(Severity::Nit, true, false),
            Delivery::Aside
        );
        assert_eq!(
            AcceptedNote::delivery(Severity::Concern, true, false),
            Delivery::Steer
        );
        assert_eq!(
            AcceptedNote::delivery(Severity::Concern, false, true),
            Delivery::Card
        );
        assert_eq!(
            AcceptedNote::delivery(Severity::Blocker, false, true),
            Delivery::FollowUp
        );
        assert_eq!(
            AcceptedNote::delivery(Severity::Blocker, true, false),
            Delivery::Steer
        );
    }

    #[test]
    fn render_line_uses_short_name() {
        let n = AcceptedNote {
            severity: Severity::Concern,
            text: "wrong API".into(),
            advisor_short: "opus".into(),
        };
        assert_eq!(n.render_line(), "[advisor opus] concern: wrong API");
    }
}
