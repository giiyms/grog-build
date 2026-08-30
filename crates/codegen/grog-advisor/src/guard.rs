//! At most one accepted note per advisor update, plus dedupe and mid-turn withhold.

use unicode_normalization::UnicodeNormalization;

use crate::note::{AcceptedNote, AdvisorNote, Severity};

/// Per-sidecar emission rules. Cleared on compaction / session rewrite.
#[derive(Debug, Default)]
pub struct EmissionGuard {
    delivered: Vec<(String, Severity)>,
    accepted_this_update: bool,
}

impl EmissionGuard {
    pub fn begin_update(&mut self) {
        self.accepted_this_update = false;
    }

    /// Accept at most one note. Mid-turn work withholds nit/concern (blocker
    /// may still interrupt). Near-identical notes are dropped unless the
    /// severity strictly escalates.
    pub fn consider(
        &mut self,
        note: AdvisorNote,
        in_progress: bool,
        advisor_short: &str,
    ) -> Option<AcceptedNote> {
        if self.accepted_this_update {
            return None;
        }
        if in_progress && note.severity < Severity::Blocker {
            return None;
        }
        let key = normalize_key(&note.text);
        if key.is_empty() {
            return None;
        }
        if let Some((_, prev)) = self.delivered.iter().find(|(k, _)| k == &key)
            && note.severity <= *prev
        {
            return None;
        }
        if self.delivered.len() >= 4096 {
            self.delivered.remove(0);
        }
        self.delivered.push((key, note.severity));
        self.accepted_this_update = true;
        Some(AcceptedNote {
            severity: note.severity,
            text: note.text,
            advisor_short: advisor_short.to_string(),
        })
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Lowercase, NFKC, collapse non-alphanumeric runs to one space.
pub fn normalize_key(text: &str) -> String {
    let nfkc: String = text.nfkc().collect();
    let mut out = String::new();
    let mut prev_space = false;
    for ch in nfkc.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            prev_space = false;
        } else if !prev_space && !out.is_empty() {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(sev: Severity, text: &str) -> AdvisorNote {
        AdvisorNote {
            severity: sev,
            text: text.into(),
        }
    }

    #[test]
    fn at_most_one_note_per_update() {
        let mut g = EmissionGuard::default();
        g.begin_update();
        assert!(g.consider(note(Severity::Nit, "unused import"), false, "opus").is_some());
        assert!(
            g.consider(note(Severity::Blocker, "tests never ran"), false, "opus").is_none(),
            "second note in the same update must drop"
        );
        g.begin_update();
        assert!(
            g.consider(note(Severity::Blocker, "tests never ran"), false, "opus").is_some()
        );
    }

    #[test]
    fn dedupe_near_identical() {
        let mut g = EmissionGuard::default();
        g.begin_update();
        assert!(g.consider(note(Severity::Nit, "Stop."), false, "luna").is_some());
        g.begin_update();
        assert!(g.consider(note(Severity::Nit, "*Stop*"), false, "luna").is_none());
        g.begin_update();
        assert!(
            g.consider(note(Severity::Concern, "Stop."), false, "luna").is_some(),
            "escalation must pass"
        );
    }

    #[test]
    fn mid_turn_withholds_nit_and_concern() {
        let mut g = EmissionGuard::default();
        g.begin_update();
        assert!(g.consider(note(Severity::Nit, "rename this"), true, "opus").is_none());
        assert!(g.consider(note(Severity::Concern, "wrong API"), true, "opus").is_none());
        assert!(g.consider(note(Severity::Blocker, "untested crash"), true, "opus").is_some());
    }

    #[test]
    fn mid_turn_allows_nit_once_turn_ended() {
        let mut g = EmissionGuard::default();
        g.begin_update();
        assert!(g.consider(note(Severity::Nit, "rename this"), false, "opus").is_some());
    }
}
