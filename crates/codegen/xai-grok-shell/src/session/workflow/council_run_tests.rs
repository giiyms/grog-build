//! Council workflow run tests. Mock the host so CI never calls Claude/agy/Codex.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use xai_workflow::{
    AgentOpts, AgentResult, Journal, WorkflowHostRequest, WorkflowOutcome, WorkflowRunParams,
    run_workflow,
};

use super::registry::BUILTIN_WORKFLOWS;

fn council_script() -> &'static str {
    BUILTIN_WORKFLOWS
        .iter()
        .find(|builtin| builtin.name == "council")
        .map(|builtin| builtin.script)
        .expect("council builtin registered")
}

fn agent_ok(output: &str) -> AgentResult {
    AgentResult {
        agent_id: "child".into(),
        success: true,
        output: serde_json::Value::String(output.into()),
        cancelled: false,
        tokens_used: 1,
        duration_ms: 1,
    }
}

fn agent_fail() -> AgentResult {
    AgentResult {
        agent_id: "child".into(),
        success: false,
        output: serde_json::Value::String(String::new()),
        cancelled: false,
        tokens_used: 0,
        duration_ms: 1,
    }
}

fn spawn_mock_host(
    mut rx: mpsc::UnboundedReceiver<WorkflowHostRequest>,
    mut on_request: impl FnMut(WorkflowHostRequest) + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Some(req) = rx.blocking_recv() {
            match req {
                WorkflowHostRequest::ReserveAgentCalls { reply, .. }
                | WorkflowHostRequest::ReleaseAgentCalls { reply, .. } => {
                    let _ = reply.send(Ok(()));
                }
                other => on_request(other),
            }
        }
    })
}

fn run_council(
    args: serde_json::Value,
    mut on_spawn: impl FnMut(&AgentOpts) -> AgentResult + Send + 'static,
) -> serde_json::Value {
    let (tx, rx) = mpsc::unbounded_channel();
    let host = spawn_mock_host(rx, move |req| match req {
        WorkflowHostRequest::SpawnAgent { opts, reply } => {
            let _ = reply.send(Ok(on_spawn(&opts)));
        }
        WorkflowHostRequest::Phase { .. } | WorkflowHostRequest::Log { .. } => {}
        other => panic!("unexpected workflow host request: {other:?}"),
    });

    let outcome = run_workflow(WorkflowRunParams {
        script: council_script().to_string(),
        args,
        journal: Journal::new(None),
        host_tx: tx,
        cancel: CancellationToken::new(),
        max_ops: WorkflowRunParams::DEFAULT_MAX_OPS,
    });
    drop(host);

    match outcome {
        WorkflowOutcome::Completed { result } => result,
        other => panic!("expected Completed, got {other:?}"),
    }
}

fn report_str(result: &serde_json::Value) -> &str {
    result
        .get("report")
        .and_then(|r| r.as_str())
        .unwrap_or_else(|| panic!("report must be a non-null string, got {result}"))
}

#[test]
fn rhai_trim_does_not_null_a_non_empty_chair_note() {
    // Isolated reproduction of the helper the workflow uses. Rhai trim()
    // returns unit; returning that used to serialize as JSON null.
    let script = r#"
        let meta = #{ name: "t", description: "d" };
        fn trimmed(s) {
            if type_of(s) != "string" {
                return "";
            }
            let out = s.trim();
            if type_of(out) == "string" {
                return out;
            }
            s
        }
        let report = trimmed("  pipeline works, but membership is degraded  ");
        complete(#{ report: report, status: "ok" });
    "#;
    let (tx, rx) = mpsc::unbounded_channel();
    let host = spawn_mock_host(rx, |req| match req {
        WorkflowHostRequest::Phase { .. } | WorkflowHostRequest::Log { .. } => {}
        other => panic!("unexpected request: {other:?}"),
    });
    let outcome = run_workflow(WorkflowRunParams {
        script: script.into(),
        args: serde_json::json!({}),
        journal: Journal::new(None),
        host_tx: tx,
        cancel: CancellationToken::new(),
        max_ops: WorkflowRunParams::DEFAULT_MAX_OPS,
    });
    drop(host);
    match outcome {
        WorkflowOutcome::Completed { result } => {
            assert_eq!(
                result.get("report").and_then(|r| r.as_str()),
                Some("pipeline works, but membership is degraded")
            );
            assert!(!result.get("report").unwrap().is_null());
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn rhai_trim_return_value_must_not_be_unit() {
    // Guard the failure mode: `return s.trim()` yields unit → JSON null.
    let script = r#"
        let meta = #{ name: "t", description: "d" };
        fn broken(s) {
            if type_of(s) == "string" {
                return s.trim();
            }
            ""
        }
        complete(#{ report: broken("  chair note  "), status: "ok" });
    "#;
    let (tx, rx) = mpsc::unbounded_channel();
    let host = spawn_mock_host(rx, |req| match req {
        WorkflowHostRequest::Phase { .. } | WorkflowHostRequest::Log { .. } => {}
        other => panic!("unexpected request: {other:?}"),
    });
    let outcome = run_workflow(WorkflowRunParams {
        script: script.into(),
        args: serde_json::json!({}),
        journal: Journal::new(None),
        host_tx: tx,
        cancel: CancellationToken::new(),
        max_ops: WorkflowRunParams::DEFAULT_MAX_OPS,
    });
    drop(host);
    match outcome {
        WorkflowOutcome::Completed { result } => {
            assert!(
                result.get("report").unwrap().is_null(),
                "documenting the bug: returning s.trim() serializes as null, got {result}"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[test]
fn degraded_membership_returns_seat_and_chair_not_null() {
    let saw_review = Arc::new(AtomicBool::new(false));
    let saw_review_flag = saw_review.clone();
    let result = run_council(
        serde_json::json!({ "query": "smoke test: does the pipeline work?" }),
        move |opts| {
            if opts
                .label
                .as_deref()
                .is_some_and(|l| l.starts_with("council-review-"))
                || opts.phase.as_deref() == Some("Review")
            {
                saw_review_flag.store(true, Ordering::SeqCst);
            }
            if opts.label.as_deref() == Some("council-chair-verdict") {
                return agent_ok("  pipeline works, but membership is degraded  ");
            }
            if opts.model.as_deref() == Some("claude-bridge/claude-opus-4-6") {
                return agent_ok("Claude's independent smoke-test answer.");
            }
            agent_fail()
        },
    );

    let report = report_str(&result);
    assert!(
        !result["report"].is_null(),
        "chair/seat product must not be JSON null: {result}"
    );
    assert!(
        report.contains("pipeline works, but membership is degraded"),
        "chair synthesis must survive trim(), got: {report}"
    );
    assert!(
        report.contains("Claude's independent smoke-test answer."),
        "the one live seat must still be visible: {report}"
    );
    assert!(
        report.contains("Skipped anonymous review"),
        "one live seat skips ranking: {report}"
    );
    assert!(
        report.contains("claude-bridge/claude-opus-4-6"),
        "named opinion must remain: {report}"
    );
    assert!(
        report.contains("Failed seats"),
        "degraded membership must note failed seats: {report}"
    );
    assert!(
        !saw_review.load(Ordering::SeqCst),
        "review stage must not run with a single live seat"
    );
}

#[test]
fn two_seats_run_ranking_then_visible_verdict() {
    let review_count = Arc::new(AtomicUsize::new(0));
    let review_flag = review_count.clone();
    let result = run_council(
        serde_json::json!({ "query": "compare two approaches" }),
        move |opts| {
            if opts
                .label
                .as_deref()
                .is_some_and(|l| l.starts_with("council-review-"))
            {
                review_flag.fetch_add(1, Ordering::SeqCst);
                assert!(
                    opts.prompt.contains("FINAL RANKING:"),
                    "review prompt must require FINAL RANKING"
                );
                return agent_ok("FINAL RANKING:\n1. Response A\n2. Response B\n");
            }
            if opts.label.as_deref() == Some("council-chair-verdict") {
                return agent_ok("Ship approach A; B is a close second.");
            }
            match opts.model.as_deref() {
                Some("claude-bridge/claude-opus-4-6") => agent_ok("Use a write-through cache."),
                Some("antigravity/gemini-3.6-flash") => agent_ok("Use a write-back cache."),
                _ => agent_fail(),
            }
        },
    );

    let report = report_str(&result);
    assert_eq!(review_count.load(Ordering::SeqCst), 2);
    assert!(report.contains("Ship approach A; B is a close second."));
    assert!(report.contains("Use a write-through cache."));
    assert!(report.contains("Use a write-back cache."));
    assert!(report.contains("FINAL RANKING:"));
    assert!(!report.contains("Skipped anonymous review"));
}

#[test]
fn chair_failure_still_returns_the_live_seat() {
    let result = run_council(
        serde_json::json!({ "query": "smoke", "members": ["claude-bridge/claude-opus-4-6"] }),
        |opts| {
            if opts.label.as_deref() == Some("council-chair-verdict") {
                return agent_fail();
            }
            agent_ok("Only seat answered.")
        },
    );
    let report = report_str(&result);
    assert!(report.contains("Only seat answered."));
    assert!(report.contains("Chair synthesis failed"));
    assert!(report.contains("Skipped anonymous review"));
}
