//! ChatGPT Codex backend consult (Responses-shaped). Not `api.openai.com`.

use crate::auth::{AUTH_ISSUER, CODEX_BACKEND, CODEX_CLIENT_ID, CodexAuth, chatgpt_account_id};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultResult {
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Codex subscription has no access token")]
    NoToken,
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("transport: {0}")]
    Transport(String),
    #[error("Codex stream: {0}")]
    Stream(String),
    #[error("Codex response had no assistant text")]
    Empty,
}

/// Honest client id on Codex HTTP. We are grog, not the official Codex CLI.
pub const ORIGINATOR: &str = "grog";

/// ChatGPT Codex `/codex/responses` `input` items.
///
/// The backend is not the OpenAI Responses API's "string or list" union: a
/// live council seat (2026-08-29) died with `HTTP 400: {"detail":"Input must
/// be a list"}` when `input` was a prompt string. Match the Codex CLI shape
/// (`type=message` + `input_text` parts).
pub fn consult_input(prompt: &str) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": prompt
        }]
    })]
}

/// Responses-shaped body for a Codex consult. `reasoning.effort` is the
/// Codex thinking flag (`xhigh` for Luna council / AskCodex).
///
/// ChatGPT Codex `/codex/responses` rejects non-streaming consults
/// (`HTTP 400: {"detail":"Stream must be set to true"}`). Codex CLI always
/// sends `stream: true` and drains the SSE. Grog does the same for a
/// one-shot consult and assembles the final assistant text.
pub fn consult_body(model: &str, prompt: &str, effort: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "input": consult_input(prompt),
        "store": false,
        "stream": true,
        "reasoning": { "effort": effort },
    })
}

pub fn consult_sync(
    auth: &CodexAuth,
    model: &str,
    prompt: &str,
    effort: Option<&str>,
) -> Result<ConsultResult, ClientError> {
    consult_at(
        &format!("{CODEX_BACKEND}/codex/responses"),
        auth,
        model,
        prompt,
        effort,
    )
}

fn consult_at(
    url: &str,
    auth: &CodexAuth,
    model: &str,
    prompt: &str,
    effort: Option<&str>,
) -> Result<ConsultResult, ClientError> {
    let tokens = auth.tokens.as_ref().ok_or(ClientError::NoToken)?;
    if tokens.access_token.is_empty() {
        return Err(ClientError::NoToken);
    }
    let account = chatgpt_account_id(auth).unwrap_or_default();
    let effort = effort.unwrap_or(crate::DEFAULT_CODEX_EFFORT);
    let body = consult_body(model, prompt, effort);
    let mut builder = reqwest::blocking::Client::new()
        .post(url)
        .bearer_auth(&tokens.access_token)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("OpenAI-Beta", "responses=experimental")
        // Honest client id: we are grog, not the official Codex CLI.
        // The backend still sees the public Codex OAuth client id and
        // ChatGPT subscription token imported from ~/.codex/auth.json.
        .header("originator", ORIGINATOR)
        .json(&body);
    if !account.is_empty() {
        builder = builder.header("ChatGPT-Account-Id", account);
    }
    let resp = builder
        .send()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    consult_http_result(status, &text)
}

fn consult_http_result(status: u16, body: &str) -> Result<ConsultResult, ClientError> {
    if !(200..300).contains(&status) {
        return Err(ClientError::Http {
            status,
            body: body.to_string(),
        });
    }
    parse_consult_stream(body)
}

pub fn refresh_sync(auth: &CodexAuth) -> Result<CodexAuth, ClientError> {
    let refresh = auth
        .tokens
        .as_ref()
        .and_then(|t| t.refresh_token.clone())
        .filter(|s| !s.is_empty())
        .ok_or(ClientError::NoToken)?;
    let url = format!("{AUTH_ISSUER}/oauth/token");
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh.as_str()),
        ("client_id", CODEX_CLIENT_ID),
    ];
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .form(&params)
        .send()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    let status = resp.status().as_u16();
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(ClientError::Http {
            status,
            body: json.to_string(),
        });
    }
    let access = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or(ClientError::Empty)?;
    let mut next = auth.clone();
    if let Some(tokens) = next.tokens.as_mut() {
        tokens.access_token = access.to_string();
        if let Some(r) = json.get("refresh_token").and_then(|v| v.as_str()) {
            tokens.refresh_token = Some(r.to_string());
        }
    }
    Ok(next)
}

/// Assemble a one-shot consult from a Codex `/codex/responses` SSE body.
///
/// Codex CLI always streams. Final text prefers `response.completed` output,
/// then `response.output_item.done` messages, then concatenated
/// `response.output_text.delta` chunks.
pub fn parse_consult_stream(body: &str) -> Result<ConsultResult, ClientError> {
    let payloads = sse_data_payloads(body);
    if payloads.is_empty() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            return extract_output_text(&json)
                .map(|text| ConsultResult { text })
                .ok_or(ClientError::Empty);
        }
        return Err(ClientError::Empty);
    }

    let mut deltas = String::new();
    let mut item_texts = Vec::new();
    let mut done_text: Option<String> = None;
    let mut completed_text: Option<String> = None;
    let mut failed: Option<String> = None;

    for payload in &payloads {
        if payload == "[DONE]" {
            break;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(payload) else {
            continue;
        };
        let kind = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match kind {
            "response.output_text.delta" => {
                if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                    deltas.push_str(delta);
                }
            }
            "response.output_item.done" => {
                if let Some(text) = event.get("item").and_then(extract_item_text) {
                    item_texts.push(text);
                }
            }
            "response.output_text.done" => {
                if let Some(text) = nonempty_trim(event.get("text").and_then(|v| v.as_str())) {
                    done_text = Some(text);
                }
            }
            "response.completed" | "response.incomplete" => {
                if let Some(text) = event.get("response").and_then(extract_output_text) {
                    completed_text = Some(text);
                }
            }
            "response.failed" => {
                failed = Some(failed_message(&event));
            }
            "error" => {
                failed = Some(
                    event
                        .pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .or_else(|| event.get("message").and_then(|v| v.as_str()))
                        .unwrap_or("error")
                        .to_string(),
                );
            }
            _ => {}
        }
    }

    if let Some(msg) = failed {
        return Err(ClientError::Stream(msg));
    }

    let text = completed_text
        .or_else(|| {
            let joined = item_texts.join("");
            nonempty_trim(Some(&joined))
        })
        .or(done_text)
        .or_else(|| nonempty_trim(Some(&deltas)))
        .ok_or(ClientError::Empty)?;
    Ok(ConsultResult { text })
}

fn sse_data_payloads(body: &str) -> Vec<String> {
    let mut events = Vec::new();
    let mut data_lines = Vec::new();
    for raw_line in body.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            if !data_lines.is_empty() {
                events.push(data_lines.join("\n"));
                data_lines.clear();
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            data_lines.push(rest.to_string());
        }
    }
    if !data_lines.is_empty() {
        events.push(data_lines.join("\n"));
    }
    events
}

fn failed_message(event: &serde_json::Value) -> String {
    event
        .pointer("/response/error/message")
        .and_then(|v| v.as_str())
        .or_else(|| event.pointer("/error/message").and_then(|v| v.as_str()))
        .unwrap_or("response.failed")
        .to_string()
}

fn nonempty_trim(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

fn extract_output_text(json: &serde_json::Value) -> Option<String> {
    if let Some(text) = nonempty_trim(json.get("output_text").and_then(|v| v.as_str())) {
        return Some(text);
    }
    let mut parts = Vec::new();
    if let Some(output) = json.get("output").and_then(|v| v.as_array()) {
        for item in output {
            append_item_text(item, &mut parts);
        }
    } else {
        append_item_text(json, &mut parts);
    }
    nonempty_trim(Some(&parts.join("")))
}

fn extract_item_text(item: &serde_json::Value) -> Option<String> {
    let mut parts = Vec::new();
    append_item_text(item, &mut parts);
    nonempty_trim(Some(&parts.join("")))
}

fn append_item_text<'a>(item: &'a serde_json::Value, parts: &mut Vec<&'a str>) {
    if item.get("type").and_then(|v| v.as_str()) == Some("reasoning") {
        return;
    }
    let Some(content) = item.get("content").and_then(|v| v.as_array()) else {
        return;
    };
    for block in content {
        let ty = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !ty.is_empty() && ty != "output_text" && ty != "text" && ty != "input_text" {
            continue;
        }
        if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
            parts.push(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::CodexTokens;

    #[test]
    fn extracts_output_text_and_content_blocks() {
        let v = serde_json::json!({"output_text":" hello "});
        assert_eq!(extract_output_text(&v).as_deref(), Some("hello"));
        let v = serde_json::json!({
            "output": [{"content": [{"text": "a"}, {"text": "b"}]}]
        });
        assert_eq!(extract_output_text(&v).as_deref(), Some("ab"));
    }

    #[test]
    fn consult_body_sends_luna_xhigh_reasoning_effort() {
        let body = consult_body("gpt-5.6-luna", "hello", crate::DEFAULT_CODEX_EFFORT);
        assert_eq!(body["model"], "gpt-5.6-luna");
        assert_eq!(body["reasoning"]["effort"], "xhigh");
        assert_ne!(body["model"], "gpt-5.3-codex");
        assert_ne!(body["model"], "gpt-5.1-codex");
    }

    #[test]
    fn consult_body_input_is_a_list_of_message_items() {
        let prompt = "What is 2+2? Reply with one sentence.";
        let body = consult_body("gpt-5.6-luna", prompt, crate::DEFAULT_CODEX_EFFORT);
        assert!(
            body["input"].is_array(),
            "ChatGPT Codex backend: HTTP 400 {{\"detail\":\"Input must be a list\"}} when input is a string"
        );
        assert!(
            !body["input"].is_string(),
            "must not send the prompt as a bare input string"
        );
        assert!(!body.as_object().unwrap().contains_key("messages"));
        let input = body["input"].as_array().expect("input list");
        assert_eq!(input.len(), 1);
        assert_eq!(input[0]["type"], "message");
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[0]["content"][0]["type"], "input_text");
        assert_eq!(input[0]["content"][0]["text"], prompt);
        assert_eq!(ORIGINATOR, "grog");
        assert_ne!(ORIGINATOR, "codex_cli_rs");
    }

    #[test]
    fn consult_body_stream_must_be_true() {
        let body = consult_body(
            "gpt-5.6-luna",
            "What is 2+2? Reply with one sentence.",
            crate::DEFAULT_CODEX_EFFORT,
        );
        assert_eq!(
            body["stream"], true,
            "live Codex 400: {{\"detail\":\"Stream must be set to true\"}}"
        );
        assert_ne!(body["stream"], false);
        assert!(body["input"].is_array());
        assert_eq!(ORIGINATOR, "grog");
    }

    #[test]
    fn parse_consult_stream_assembles_mocked_sse_into_text() {
        let sse = mock_codex_sse("2 + 2 equals 4.");
        let out = parse_consult_stream(&sse).expect("stream text");
        assert_eq!(out.text, "2 + 2 equals 4.");
    }

    #[test]
    fn parse_consult_stream_uses_output_text_deltas_when_completed_has_no_output() {
        let sse = format!(
            "event: response.output_text.delta\ndata: {}\n\n\
event: response.output_text.delta\ndata: {}\n\n\
event: response.completed\ndata: {}\n\ndata: [DONE]\n\n",
            serde_json::json!({"type":"response.output_text.delta","delta":"2 + 2"}),
            serde_json::json!({"type":"response.output_text.delta","delta":" equals 4."}),
            serde_json::json!({"type":"response.completed","response":{"id":"resp_1"}}),
        );
        let out = parse_consult_stream(&sse).expect("delta text");
        assert_eq!(out.text, "2 + 2 equals 4.");
    }

    #[test]
    fn parse_consult_stream_skips_reasoning_items() {
        let sse = format!(
            "event: response.output_item.done\ndata: {}\n\n\
event: response.output_item.done\ndata: {}\n\n\
data: [DONE]\n\n",
            serde_json::json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "reasoning",
                    "content": [{"type": "output_text", "text": "thinking"}]
                }
            }),
            serde_json::json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "2 + 2 equals 4."}]
                }
            }),
        );
        let out = parse_consult_stream(&sse).expect("message text");
        assert_eq!(out.text, "2 + 2 equals 4.");
    }

    #[test]
    fn parse_consult_stream_reports_failed_event() {
        let sse = format!(
            "event: response.failed\ndata: {}\n\n",
            serde_json::json!({
                "type": "response.failed",
                "response": {"error": {"message": "rate limited"}}
            })
        );
        let err = parse_consult_stream(&sse).unwrap_err();
        assert!(matches!(err, ClientError::Stream(msg) if msg.contains("rate limited")));
    }

    #[test]
    fn http_400_stream_must_be_true_is_not_parsed_as_success() {
        let err =
            consult_http_result(400, r#"{"detail":"Stream must be set to true"}"#).unwrap_err();
        match err {
            ClientError::Http { status, body } => {
                assert_eq!(status, 400);
                assert!(body.contains("Stream must be set to true"));
            }
            other => panic!("expected HTTP 400, got {other:?}"),
        }
    }

    #[test]
    fn consult_sync_parses_mocked_stream_response() {
        let mut server = mockito::Server::new();
        let sse = mock_codex_sse("2 + 2 equals 4.");
        let mock = server
            .mock("POST", "/codex/responses")
            .match_header("originator", "grog")
            .match_header("accept", "text/event-stream")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "stream": true,
                "store": false,
                "model": "gpt-5.6-luna",
            })))
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse)
            .create();

        let auth = CodexAuth {
            tokens: Some(CodexTokens {
                access_token: "tok".into(),
                refresh_token: None,
                id_token: None,
                account_id: Some("acct-1".into()),
            }),
            last_refresh: None,
            openai_api_key: None,
        };
        let url = format!("{}/codex/responses", server.url());
        let out = consult_at(&url, &auth, "gpt-5.6-luna", "What is 2+2?", Some("xhigh"))
            .expect("mocked consult");
        assert_eq!(out.text, "2 + 2 equals 4.");
        mock.assert();
    }

    #[test]
    fn consult_sync_mocked_request_sends_input_list_and_stream_true() {
        let mut server = mockito::Server::new();
        let prompt = "What is 2+2? Reply with one sentence.";
        let sse = mock_codex_sse("2 + 2 equals 4.");
        let mock = server
            .mock("POST", "/codex/responses")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "stream": true,
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": prompt}]
                }]
            })))
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse)
            .create();

        let auth = CodexAuth {
            tokens: Some(CodexTokens {
                access_token: "tok".into(),
                refresh_token: None,
                id_token: None,
                account_id: None,
            }),
            last_refresh: None,
            openai_api_key: None,
        };
        let url = format!("{}/codex/responses", server.url());
        let out = consult_at(&url, &auth, "gpt-5.6-luna", prompt, None).expect("mocked consult");
        assert_eq!(out.text, "2 + 2 equals 4.");
        mock.assert();
    }

    fn mock_codex_sse(text: &str) -> String {
        let created = serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp_test", "status": "in_progress", "output": []}
        });
        let delta = serde_json::json!({
            "type": "response.output_text.delta",
            "delta": text
        });
        let item = serde_json::json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text}]
            }
        });
        let completed = serde_json::json!({
            "type": "response.completed",
            "response": {
                "id": "resp_test",
                "status": "completed",
                "output_text": text,
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": text}]
                }]
            }
        });
        format!(
            "event: response.created\ndata: {created}\n\n\
event: response.output_text.delta\ndata: {delta}\n\n\
event: response.output_item.done\ndata: {item}\n\n\
event: response.completed\ndata: {completed}\n\n\
data: [DONE]\n\n"
        )
    }
}
