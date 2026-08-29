use xai_grok_sampling_types::{SearchDateBound, ToolOverrides, WebSearchOptions, XSearchOptions};

use super::{
    CLASSIFIER_REQUEST_TOKEN_RESERVE, classifier_request_fits_context, grog_native_consult_id,
    grog_native_uses_isolated_ask, native_catalog_slug, resolve_configured_cutoff,
    tool_can_mutate_workspace,
};

fn x_cut(to: &str) -> XSearchOptions {
    XSearchOptions {
        date_bound: Some(SearchDateBound::new(None, Some(to.into())).unwrap()),
    }
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
