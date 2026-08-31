use xai_grok_sampling_types::{SearchDateBound, ToolOverrides, WebSearchOptions, XSearchOptions};

use super::{
    CLASSIFIER_REQUEST_TOKEN_RESERVE, LengthSalvageAction, LengthSalvageStreak,
    MAX_OUTPUT_TOKEN_LIMIT_RETRIES, classifier_request_fits_context, grog_native_consult_id,
    grog_native_uses_isolated_ask, native_catalog_slug, resolve_configured_cutoff,
    tool_can_mutate_workspace,
};

fn x_cut(to: &str) -> XSearchOptions {
    XSearchOptions {
        date_bound: Some(SearchDateBound::new(None, Some(to.into())).unwrap()),
    }
}

/// A prompt whose every sample ends Length-with-tools salvages up to the
/// cap (reminder only on the first) and then fails the turn.
#[test]
fn length_salvage_streak_proceeds_to_the_cap_then_exhausts() {
    let mut streak = LengthSalvageStreak::default();
    for n in 1..=MAX_OUTPUT_TOKEN_LIMIT_RETRIES {
        match streak.on_sample(true) {
            LengthSalvageAction::Proceed { inject_reminder } => {
                assert_eq!(
                    inject_reminder,
                    n == 1,
                    "reminder only on the first salvage"
                );
            }
            other => panic!("salvage {n} within the cap must proceed, got {other:?}"),
        }
    }
    assert!(matches!(
        streak.on_sample(true),
        LengthSalvageAction::Exhausted
    ));
}

/// A non-Length (or tool-less) sample resets the streak: a later salvage
/// starts a fresh streak and re-injects the reminder.
#[test]
fn length_salvage_streak_resets_on_non_length_sample() {
    let mut streak = LengthSalvageStreak::default();
    for _ in 0..MAX_OUTPUT_TOKEN_LIMIT_RETRIES {
        let _ = streak.on_sample(true);
    }
    assert!(matches!(
        streak.on_sample(false),
        LengthSalvageAction::NotSalvage
    ));
    assert!(matches!(
        streak.on_sample(true),
        LengthSalvageAction::Proceed {
            inject_reminder: true
        }
    ));
}

#[test]
fn classifier_request_bound_enforces_its_reserve_with_saturating_arithmetic() {
    let window = 12_000 + CLASSIFIER_REQUEST_TOKEN_RESERVE;
    for (input, context_window, expected) in [
        (12_000, window, true),
        (12_001, window, false),
        (u64::MAX, u64::MAX, false),
    ] {
        assert_eq!(
            classifier_request_fits_context(input, context_window),
            expected
        );
    }
}

#[test]
fn seed_cutoff_is_inherited_without_a_per_turn_update() {
    let seed = ToolOverrides {
        x_search: Some(x_cut("2020-01-01")),
        web_search: None,
    };
    assert_eq!(resolve_configured_cutoff(Some(seed.clone()), None), seed);
}

#[test]
fn non_empty_base_cutoff_wins_per_tool_and_an_empty_one_reverts_to_the_seed() {
    let seed = ToolOverrides {
        x_search: Some(x_cut("2020-01-01")),
        web_search: Some(WebSearchOptions {
            allowed_domains: Some(vec!["x.com".into()]),
            excluded_domains: None,
        }),
    };
    let base = ToolOverrides {
        x_search: Some(x_cut("2019-06-01")),
        web_search: Some(WebSearchOptions {
            allowed_domains: Some(vec![]),
            excluded_domains: None,
        }),
    };
    let got = resolve_configured_cutoff(Some(seed.clone()), Some(&base));
    assert_eq!(got.x_search, Some(x_cut("2019-06-01")));
    assert_eq!(got.web_search, seed.web_search);
}

#[test]
fn inherited_cutoff_agrees_with_the_wire_echo_so_the_two_implementations_cannot_drift() {
    use xai_grok_sampling_types::{HostedTool, apply_tool_overrides};
    let web = WebSearchOptions {
        allowed_domains: Some(vec!["x.com".into()]),
        excluded_domains: None,
    };
    let cases = [
        (
            Some(ToolOverrides {
                x_search: Some(x_cut("2020-01-01")),
                web_search: None,
            }),
            None,
        ),
        (
            Some(ToolOverrides {
                x_search: Some(x_cut("2020-01-01")),
                web_search: Some(web.clone()),
            }),
            Some(ToolOverrides {
                x_search: Some(x_cut("2019-06-01")),
                web_search: None,
            }),
        ),
        (
            None,
            Some(ToolOverrides {
                x_search: Some(x_cut("2018-01-01")),
                web_search: Some(web.clone()),
            }),
        ),
    ];
    for (seed, base) in cases {
        let mut tools = vec![
            HostedTool::WebSearch { options: None },
            HostedTool::XSearch { options: None },
        ];
        apply_tool_overrides(&mut tools, seed.as_ref());
        let wire_echo = apply_tool_overrides(&mut tools, base.as_ref());
        let inherited = resolve_configured_cutoff(seed.clone(), base.as_ref());
        assert_eq!(wire_echo, inherited, "seed={seed:?} base={base:?}");
    }
}

#[test]
fn read_only_native_turns_use_isolated_ask() {
    assert!(grog_native_uses_isolated_ask(&[]));
    assert!(grog_native_uses_isolated_ask(&[
        spec("read_file"),
        spec("grep")
    ]));
    assert!(!grog_native_uses_isolated_ask(&[
        spec("read_file"),
        spec("search_replace"),
    ]));
    assert!(!grog_native_uses_isolated_ask(&[spec(
        "run_terminal_command"
    )]));
    assert!(tool_can_mutate_workspace("GrokBuild:search_replace"));
    assert!(!tool_can_mutate_workspace("read_file"));
}

fn spec(name: &str) -> xai_grok_sampling_types::ToolSpec {
    xai_grok_sampling_types::ToolSpec {
        name: name.into(),
        description: None,
        parameters: serde_json::json!({}),
    }
}

#[test]
fn council_codex_slug_is_not_http_when_paired_with_grog_marker() {
    assert_eq!(
        grog_native_consult_id(Some("codex/gpt-5.6-luna"), None).as_deref(),
        Some("codex/gpt-5.6-luna")
    );
    assert_eq!(
        grog_native_consult_id(Some("gpt-5.6-luna"), None),
        None,
        "slug alone must not look native — that was the spawn bug"
    );
    assert!(native_catalog_slug("gpt-5.6-luna"));
    assert_eq!(
        grog_native_consult_id(Some("gpt-5.6-luna"), Some("grog://codex")).as_deref(),
        Some("codex/gpt-5.6-luna")
    );
    let route = grog_providers::inference_route("gpt-5.6-luna", "grog://codex");
    assert!(route.is_native());
    assert_eq!(route.sampler_chat_completions_url("grog://codex"), None);
}

#[test]
fn claude_and_antigravity_slugs_are_native_without_waiting_on_grog_url() {
    assert_eq!(
        grog_native_consult_id(Some("claude-opus-5"), None).as_deref(),
        Some("claude-opus-5")
    );
    assert_eq!(
        grog_native_consult_id(Some("gemini-3.7-flash-high"), None).as_deref(),
        Some("gemini-3.7-flash-high")
    );
    assert_eq!(
        grog_native_consult_id(Some("grok-4"), Some("https://api.x.ai/v1")),
        None
    );
}

#[cfg(test)]
mod subagent_sampling_gate_tests {
    use super::super::super::support::create_test_actor;
    use super::super::acquire_subagent_sampling_permit;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::Semaphore;

    #[derive(Default)]
    struct ConcurrencyProbe {
        in_flight: AtomicUsize,
        max_in_flight: AtomicUsize,
    }

    impl ConcurrencyProbe {
        fn enter(&self) {
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        }
        fn leave(&self) {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn subagent_submits_never_exceed_cap_and_excess_queues() {
        const CAP: usize = 3;
        const TURNS: usize = 12;
        let semaphore = Arc::new(Semaphore::new(CAP));
        let probe = Arc::new(ConcurrencyProbe::default());
        let ran = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..TURNS {
            let gate = Some(semaphore.clone());
            let probe = probe.clone();
            let ran = ran.clone();
            handles.push(tokio::spawn(async move {
                let permit = acquire_subagent_sampling_permit(&gate).await;
                assert!(permit.is_some(), "a subagent turn must receive a permit");
                probe.enter();
                tokio::time::sleep(Duration::from_millis(20)).await;
                probe.leave();
                ran.fetch_add(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(
            ran.load(Ordering::SeqCst),
            TURNS,
            "every queued turn ran (queued, not errored)"
        );
        assert!(
            probe.max_in_flight.load(Ordering::SeqCst) <= CAP,
            "in-flight subagent submits exceeded the cap: {} > {CAP}",
            probe.max_in_flight.load(Ordering::SeqCst),
        );
    }

    #[tokio::test]
    async fn cancelled_waiter_releases_without_deadlock() {
        let semaphore = Arc::new(Semaphore::new(1));
        let gate = Some(semaphore.clone());
        let held = acquire_subagent_sampling_permit(&gate).await;
        assert!(held.is_some());

        let waiter = tokio::spawn({
            let gate = gate.clone();
            async move {
                let _permit = acquire_subagent_sampling_permit(&gate).await;
                std::future::pending::<()>().await;
            }
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(
            semaphore.available_permits(),
            0,
            "slot stays held while the second turn queues"
        );
        waiter.abort();
        let _ = waiter.await;

        drop(held);
        let next = tokio::time::timeout(
            Duration::from_millis(200),
            acquire_subagent_sampling_permit(&gate),
        )
        .await
        .expect("a permit must be free once the held one is released");
        assert!(next.is_some(), "the cancelled waiter did not leak the slot");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn submit_holds_permit_for_subagent_not_main() {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async {
                let saturated = Arc::new(Semaphore::new(1));
                let _held = saturated.clone().acquire_owned().await.unwrap();

                let (gw_tx, _gw_rx) = tokio::sync::mpsc::unbounded_channel();
                let (p_tx, _p_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut subagent = create_test_actor(0, 200_000, 80, gw_tx, p_tx).await;
                subagent.sampling_gate = Some(saturated.clone());
                let subagent = Arc::new(subagent);

                let queued = tokio::time::timeout(
                    Duration::from_millis(150),
                    subagent.submit_turn_request(Default::default()),
                )
                .await;
                assert!(
                    queued.is_err(),
                    "a subagent submit must queue behind the drained gate, never reaching the sampler"
                );

                let (gw_tx, _gw_rx) = tokio::sync::mpsc::unbounded_channel();
                let (p_tx, _p_rx) = tokio::sync::mpsc::unbounded_channel();
                let main = create_test_actor(0, 200_000, 80, gw_tx, p_tx).await;
                assert!(main.sampling_gate.is_none());
                let main = Arc::new(main);

                let ran = tokio::time::timeout(
                    Duration::from_millis(150),
                    main.submit_turn_request(Default::default()),
                )
                .await;
                assert!(
                    ran.is_ok(),
                    "the main session must reach the sampler even while the gate is drained"
                );
                assert!(
                    main.turn_stream_drained.lock().is_empty(),
                    "a result with no queued terminal event must release request ownership before returning"
                );
            })
            .await;
    }
}
