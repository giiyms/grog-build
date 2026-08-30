//! `/advisor` sidecar: slash handling, consult spawn, and note injection.

use grog_advisor::{
    AdvisorVerb, Delivery, SeatSource, parse_verb, prefer_config_or_complement, resolve_spec,
    seat_readiness,
};
use grog_advisor::{TranscriptItem, is_advisor_injected};

use super::*;

impl SessionActor {
    pub(super) async fn handle_advisor_slash(&self, args: &str) {
        if self.startup_hints.is_subagent {
            self.send_host_turn_slash_command_output(
                "/advisor is not available in subagent sessions.",
            )
            .await;
            return;
        }
        let verb = match parse_verb(args) {
            Ok(v) => v,
            Err(e) => {
                self.send_host_turn_slash_command_output(&e).await;
                return;
            }
        };
        let primary = self.current_model_id().await;
        let transcript_len = self.chat_state_handle.get_conversation_len().await;
        match verb {
            AdvisorVerb::Toggle => {
                let cfg = crate::util::config::load_config().await;
                let (fallback, source) = prefer_config_or_complement(
                    cfg.models.advisor.as_deref(),
                    effort_token_from_config(cfg.models.advisor_reasoning_effort.as_ref()),
                    &primary,
                );
                let enabled = {
                    let mut st = self.advisor.lock();
                    if st.seat.is_none() && !st.enabled {
                        st.enable_with_source(fallback, transcript_len, source);
                        true
                    } else {
                        st.toggle(fallback, transcript_len)
                    }
                };
                let msg = if enabled {
                    let st = self.advisor.lock();
                    let seat = st.seat.as_ref();
                    format!(
                        "advisor enabled: {} (`{}`) [{}]",
                        seat.map(|s| s.display_name.as_str()).unwrap_or("?"),
                        seat.map(|s| s.qualified.as_str()).unwrap_or("?"),
                        st.seat_source.status_label()
                    )
                } else {
                    "advisor disabled (session-scoped; models.advisor is unchanged)".into()
                };
                self.send_host_turn_slash_command_output(&msg).await;
            }
            AdvisorVerb::On { spec } => {
                let outcome = self
                    .advisor_enable(spec.as_ref(), &primary, transcript_len)
                    .await;
                self.send_host_turn_slash_command_output(&outcome).await;
            }
            AdvisorVerb::Off => {
                self.advisor.lock().disable();
                self.send_host_turn_slash_command_output(
                    "advisor disabled (session-scoped; models.advisor is unchanged)",
                )
                .await;
            }
            AdvisorVerb::Status => {
                let cfg = crate::util::config::load_config().await;
                let st = self.advisor.lock();
                let readiness = st.seat.as_ref().and_then(seat_readiness);
                let mut body = st.status_lines(&primary, readiness.as_deref());
                if let Some(raw) = cfg.models.advisor.as_deref() {
                    body.push_str(&format!("\nconfig models.advisor = {raw}"));
                }
                drop(st);
                self.send_host_turn_slash_command_output(&body).await;
            }
            AdvisorVerb::Dump => {
                let dump = self.advisor.lock().dump_notes();
                self.send_host_turn_slash_command_output(&dump).await;
            }
            AdvisorVerb::OpenPicker | AdvisorVerb::Cycle => {
                let next = {
                    let mut st = self.advisor.lock();
                    let next = grog_advisor::cycle_seat(st.seat.as_ref(), &primary);
                    st.set_seat(next.clone());
                    if st.enabled {
                        st.enable(next.clone(), transcript_len);
                    }
                    next
                };
                let _ = crate::util::config::set_advisor_model(next.qualified.clone()).await;
                persist_advisor_effort(next.effort.as_deref()).await;
                self.send_host_turn_slash_command_output(&format!(
                    "advisor model: {} (`{}`) [session override]\n(TUI: /advisor model opens the picker)",
                    next.display_name, next.qualified
                ))
                .await;
            }
            AdvisorVerb::Set(spec) => {
                let outcome = self
                    .advisor_set_and_enable(&spec.raw, spec.effort.as_deref(), &primary, transcript_len)
                    .await;
                self.send_host_turn_slash_command_output(&outcome).await;
            }
        }
    }

    async fn advisor_enable(
        &self,
        spec: Option<&grog_advisor::ModelSpec>,
        primary: &str,
        transcript_len: usize,
    ) -> String {
        match spec {
            Some(spec) => {
                self.advisor_set_and_enable(&spec.raw, spec.effort.as_deref(), primary, transcript_len)
                    .await
            }
            None => {
                let cfg = crate::util::config::load_config().await;
                let (seat, source) = prefer_config_or_complement(
                    cfg.models.advisor.as_deref(),
                    effort_token_from_config(cfg.models.advisor_reasoning_effort.as_ref()),
                    primary,
                );
                self.advisor.lock().enable_with_source(seat.clone(), transcript_len, source);
                format!(
                    "advisor enabled: {} (`{}`) [{}]",
                    seat.display_name,
                    seat.qualified,
                    source.status_label()
                )
            }
        }
    }

    async fn advisor_set_and_enable(
        &self,
        raw: &str,
        effort: Option<&str>,
        _primary: &str,
        transcript_len: usize,
    ) -> String {
        let seat = match resolve_spec(raw, effort) {
            Ok(s) => s,
            Err(e) => return e.to_string(),
        };
        let _ = crate::util::config::set_advisor_model(seat.qualified.clone()).await;
        persist_advisor_effort(seat.effort.as_deref()).await;
        self.advisor
            .lock()
            .enable_with_source(seat.clone(), transcript_len, SeatSource::SessionOverride);
        format!(
            "advisor enabled: {} (`{}`) [session override]",
            seat.display_name, seat.qualified
        )
    }

    pub(super) async fn maybe_enqueue_advisor_review(&self, in_progress: bool) {
        if self.startup_hints.is_subagent {
            return;
        }
        let enabled = self.advisor.lock().enabled;
        if !enabled {
            return;
        }
        let conv = self.chat_state_handle.get_conversation().await;
        let items = transcript_items_from_conversation(&conv);
        let job = self.advisor.lock().take_review_job(&items, in_progress);
        let Some(job) = job else {
            return;
        };
        let tx = self.session_cmd_tx.clone();
        tokio::spawn(async move {
            let result = grog_providers::consult::ask_with_effort(
                &job.qualified,
                &job.prompt,
                job.effort.as_deref(),
            )
            .await;
            let output = match result {
                Ok(o) => Ok(o.text),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(SessionCommand::AdvisorReview {
                generation: job.generation,
                output,
                in_progress: job.in_progress,
            });
        });
    }

    pub(super) async fn handle_advisor_review(
        &self,
        generation: u64,
        output: Result<String, String>,
        in_progress: bool,
    ) {
        let output = match output {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!(error = %err, "advisor consult failed (primary continues)");
                self.advisor.lock().finish_failed(generation);
                return;
            }
        };
        let note = self.advisor.lock().accept_output(generation, &output, in_progress);
        let Some(note) = note else {
            return;
        };
        if is_advisor_injected(&note.text) {
            return;
        }
        let line = note.render_line();
        let turn_running = self
            .tool_context
            .is_turn_active
            .as_ref()
            .map(|f| f.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false);
        let delivery = grog_advisor::AcceptedNote::delivery(note.severity, turn_running, !turn_running);
        match delivery {
            Delivery::Steer | Delivery::Aside => {
                self.pending_interjections.push(PendingInterjection {
                    text: line,
                    attachments: Vec::new(),
                });
            }
            Delivery::Card => {
                self.push_system_reminder(&line);
                self.send_host_turn_slash_command_output(&line).await;
            }
            Delivery::FollowUp => {
                self.queue_interjection_fallback_prompt(line, Vec::new(), false)
                    .await;
            }
        }
    }
}

fn transcript_items_from_conversation(conv: &[ConversationItem]) -> Vec<TranscriptItem> {
    conv.iter()
        .filter_map(|item| match item {
            ConversationItem::User(u) => Some(TranscriptItem {
                role: "user",
                text: content_parts_text(&u.content),
                tool_name: None,
            }),
            ConversationItem::Assistant(a) => {
                let tool_name = a.tool_calls.first().map(|t| t.name.clone());
                Some(TranscriptItem {
                    role: "assistant",
                    text: a.content.to_string(),
                    tool_name,
                })
            }
            ConversationItem::ToolResult(t) => Some(TranscriptItem {
                role: "tool",
                text: t.content.to_string(),
                tool_name: Some(t.tool_call_id.clone()),
            }),
            _ => None,
        })
        .collect()
}

fn content_parts_text(parts: &[ContentPart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            ContentPart::Text { text } => Some(text.as_ref()),
            ContentPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn effort_token_from_config(
    effort: Option<&crate::sampling::types::ReasoningEffort>,
) -> Option<&'static str> {
    use crate::sampling::types::ReasoningEffort;
    effort.map(|e| match e {
        ReasoningEffort::None => "none",
        ReasoningEffort::Minimal => "low",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::Xhigh => "xhigh",
        ReasoningEffort::Max => "max",
    })
}

async fn persist_advisor_effort(token: Option<&str>) {
    let parsed = token.and_then(crate::util::config::reasoning_effort_from_token);
    let _ = crate::util::config::set_advisor_reasoning_effort(parsed).await;
}
