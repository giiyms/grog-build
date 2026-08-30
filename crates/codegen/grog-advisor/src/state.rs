//! Session-scoped advisor cursor, enable flag, and review job.

use crate::delta::{TranscriptItem, render_delta};
use crate::guard::EmissionGuard;
use crate::note::{AcceptedNote, AdvisorNote, parse_advisor_output};
use crate::persist::SeatSource;
use crate::prompt::{build_review_prompt, system_prompt};
use crate::seats::{AdvisorSeat, complement_seat};

/// Live sidecar state. Enable/disable is session-only.
#[derive(Debug, Default)]
pub struct AdvisorState {
    pub enabled: bool,
    pub seat: Option<AdvisorSeat>,
    pub seat_source: SeatSource,
    pub cursor: usize,
    pub generation: u64,
    pub last_note: Option<AcceptedNote>,
    pub notes: Vec<AcceptedNote>,
    pub guard: EmissionGuard,
    pub in_flight: bool,
}

/// One background consult. Failures must not kill the primary turn.
#[derive(Debug, Clone)]
pub struct ReviewJob {
    pub generation: u64,
    pub qualified: String,
    pub effort: Option<String>,
    pub prompt: String,
    pub in_progress: bool,
    pub advisor_short: String,
}

impl AdvisorState {
    /// Seed the cursor to `len` so enabling mid-session does not replay history.
    /// Marks the seat as a session override (slash / picker commit).
    pub fn enable(&mut self, seat: AdvisorSeat, transcript_len: usize) {
        self.enable_with_source(seat, transcript_len, SeatSource::SessionOverride);
    }

    pub fn enable_from_config(&mut self, seat: AdvisorSeat, transcript_len: usize) {
        self.enable_with_source(seat, transcript_len, SeatSource::Config);
    }

    pub fn enable_with_source(
        &mut self,
        seat: AdvisorSeat,
        transcript_len: usize,
        source: SeatSource,
    ) {
        self.enabled = true;
        self.seat = Some(seat);
        self.seat_source = source;
        self.cursor = transcript_len;
        self.generation = self.generation.wrapping_add(1);
        self.in_flight = false;
        self.guard.begin_update();
    }

    pub fn disable(&mut self) {
        self.enabled = false;
        self.generation = self.generation.wrapping_add(1);
        self.in_flight = false;
    }

    pub fn toggle(&mut self, fallback_seat: AdvisorSeat, transcript_len: usize) -> bool {
        if self.enabled {
            self.disable();
            false
        } else {
            match self.seat.clone() {
                Some(seat) => {
                    let source = if self.seat_source == SeatSource::None {
                        SeatSource::SessionOverride
                    } else {
                        self.seat_source
                    };
                    self.enable_with_source(seat, transcript_len, source);
                }
                None => {
                    self.enable_with_source(
                        fallback_seat,
                        transcript_len,
                        SeatSource::Complement,
                    );
                }
            }
            true
        }
    }

    /// Compaction / session rewrite: do not replay pre-rewrite history.
    pub fn reset_after_rewrite(&mut self) {
        self.cursor = 0;
        self.generation = self.generation.wrapping_add(1);
        self.in_flight = false;
        self.guard.reset();
    }

    pub fn set_seat(&mut self, seat: AdvisorSeat) {
        self.set_seat_with_source(seat, SeatSource::SessionOverride);
    }

    pub fn set_seat_with_source(&mut self, seat: AdvisorSeat, source: SeatSource) {
        self.seat = Some(seat);
        self.seat_source = source;
    }

    /// Build a consult job from new transcript items. `None` if disabled,
    /// already in flight, or the delta is empty after stripping self-notes.
    pub fn take_review_job(
        &mut self,
        items: &[TranscriptItem],
        in_progress: bool,
    ) -> Option<ReviewJob> {
        if !self.enabled || self.in_flight {
            return None;
        }
        let seat = self.seat.clone()?;
        let (delta, new_cursor) = render_delta(items, self.cursor);
        self.cursor = new_cursor;
        if delta.trim().is_empty() {
            return None;
        }
        self.in_flight = true;
        self.guard.begin_update();
        let user = build_review_prompt(&delta, in_progress);
        Some(ReviewJob {
            generation: self.generation,
            qualified: seat.qualified.clone(),
            effort: seat.effort_token(),
            prompt: format!("{}\n\n{user}", system_prompt()),
            in_progress,
            advisor_short: seat.short_name,
        })
    }

    pub fn accept_output(&mut self, generation: u64, output: &str, in_progress: bool) -> Option<AcceptedNote> {
        if generation != self.generation || !self.enabled {
            self.in_flight = false;
            return None;
        }
        self.in_flight = false;
        let note: AdvisorNote = parse_advisor_output(output)?;
        let short = self
            .seat
            .as_ref()
            .map(|s| s.short_name.as_str())
            .unwrap_or("advisor");
        let accepted = self.guard.consider(note, in_progress, short)?;
        self.last_note = Some(accepted.clone());
        self.notes.push(accepted.clone());
        Some(accepted)
    }

    pub fn finish_failed(&mut self, generation: u64) {
        if generation == self.generation {
            self.in_flight = false;
        }
    }

    pub fn dump_notes(&self) -> String {
        if self.notes.is_empty() {
            return "(no advisor notes this session)".into();
        }
        self.notes
            .iter()
            .map(AcceptedNote::render_line)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn status_lines(&self, primary_model: &str, readiness: Option<&str>) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "advisor: {}",
            if self.enabled { "enabled" } else { "disabled (session-scoped)" }
        ));
        match &self.seat {
            Some(seat) => {
                lines.push(format!(
                    "model: {} (`{}`)",
                    seat.display_name, seat.qualified
                ));
                if let Some(effort) = seat.effort_token() {
                    lines.push(format!("thinking/effort: {effort}"));
                }
                lines.push(format!("source: {}", self.seat_source.status_label()));
            }
            None => {
                let fallback = complement_seat(primary_model);
                lines.push(format!(
                    "model: (none yet; /advisor on would pick {} — `{}`)",
                    fallback.display_name, fallback.qualified
                ));
            }
        }
        if let Some(detail) = readiness {
            lines.push(format!("provider: unavailable — {detail}"));
        }
        if let Some(note) = &self.last_note {
            lines.push(format!("last note: {}", note.render_line()));
        } else {
            lines.push("last note: (none)".into());
        }
        lines.push("This is not /council and not an oh-my-pi wrap.".into());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::TranscriptItem;
    use crate::persist::SeatSource;
    use crate::seats::{luna, opus};

    fn item(role: &'static str, text: &str) -> TranscriptItem {
        TranscriptItem {
            role,
            text: text.into(),
            tool_name: None,
        }
    }

    #[test]
    fn toggle_is_session_scoped_and_seeds_cursor() {
        let mut s = AdvisorState::default();
        let items = vec![item("user", "old"), item("assistant", "old reply")];
        assert!(s.toggle(opus(), items.len()));
        assert!(s.enabled);
        assert_eq!(s.cursor, 2);
        assert!(!s.toggle(opus(), items.len()));
        assert!(!s.enabled);
    }

    #[test]
    fn slash_set_is_session_override_not_config() {
        let mut s = AdvisorState::default();
        s.enable(luna(), 0);
        assert_eq!(s.seat_source, SeatSource::SessionOverride);
        let status = s.status_lines("grok-4", None);
        assert!(status.contains("session override"), "{status}");
        assert!(!status.contains("models.advisor"));
    }

    #[test]
    fn enable_from_config_status_names_models_advisor() {
        let mut s = AdvisorState::default();
        s.enable_from_config(luna(), 0);
        let status = s.status_lines("grok-4", None);
        assert!(status.contains("models.advisor"), "{status}");
        assert!(!status.contains("session override"));
    }

    #[test]
    fn enable_mid_session_does_not_replay() {
        let mut s = AdvisorState::default();
        let mut items = vec![item("user", "ancient")];
        s.enable(luna(), items.len());
        items.push(item("assistant", "new work"));
        let job = s.take_review_job(&items, true).unwrap();
        assert!(!job.prompt.contains("ancient"));
        assert!(job.prompt.contains("new work"));
    }

    #[test]
    fn rewrite_resets_cursor_and_drops_inflight() {
        let mut s = AdvisorState::default();
        s.enable(luna(), 4);
        s.in_flight = true;
        s.reset_after_rewrite();
        assert_eq!(s.cursor, 0);
        assert!(!s.in_flight);
    }

    #[test]
    fn accept_strips_self_review_and_one_note() {
        let mut s = AdvisorState::default();
        s.enable(opus(), 0);
        let items = vec![
            item("assistant", "editing src/a.rs"),
            item("user", "[advisor opus] concern: stale"),
        ];
        let job = s.take_review_job(&items, false).unwrap();
        assert!(!job.prompt.contains("[advisor opus]"));
        let n = s
            .accept_output(
                job.generation,
                r#"{"severity":"nit","note":"rename foo"}"#,
                false,
            )
            .unwrap();
        assert_eq!(n.severity, crate::note::Severity::Nit);
        assert!(
            s.accept_output(job.generation, r#"{"severity":"blocker","note":"x"}"#, false)
                .is_none()
        );
    }
}
