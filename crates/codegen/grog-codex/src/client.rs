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
pub fn consult_body(model: &str, prompt: &str, effort: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "input": consult_input(prompt),
        "store": false,
        "stream": false,
        "reasoning": { "effort": effort },
    })
}

pub fn consult_sync(
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
    let url = format!("{CODEX_BACKEND}/codex/responses");
    let effort = effort.unwrap_or(crate::DEFAULT_CODEX_EFFORT);
    let body = consult_body(model, prompt, effort);
    let mut builder = reqwest::blocking::Client::new()
        .post(&url)
        .bearer_auth(&tokens.access_token)
        .header("Content-Type", "application/json")
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
    let json: serde_json::Value = resp
        .json()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    if !(200..300).contains(&status) {
        return Err(ClientError::Http {
            status,
            body: json.to_string(),
        });
    }
    let text = extract_output_text(&json).ok_or(ClientError::Empty)?;
    Ok(ConsultResult { text })
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

fn extract_output_text(json: &serde_json::Value) -> Option<String> {
    if let Some(s) = json.get("output_text").and_then(|v| v.as_str()) {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let mut parts = Vec::new();
    if let Some(output) = json.get("output").and_then(|v| v.as_array()) {
        for item in output {
            if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
                for block in content {
                    if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                        parts.push(t);
                    }
                }
            }
        }
    }
    let joined = parts.join("");
    let joined = joined.trim();
    if joined.is_empty() {
        None
    } else {
        Some(joined.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
