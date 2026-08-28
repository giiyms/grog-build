//! ChatGPT Codex backend consult (Responses-shaped). Not `api.openai.com`.

use crate::auth::{chatgpt_account_id, CodexAuth, AUTH_ISSUER, CODEX_BACKEND, CODEX_CLIENT_ID};

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

pub fn consult_sync(auth: &CodexAuth, model: &str, prompt: &str) -> Result<ConsultResult, ClientError> {
    let tokens = auth.tokens.as_ref().ok_or(ClientError::NoToken)?;
    if tokens.access_token.is_empty() {
        return Err(ClientError::NoToken);
    }
    let account = chatgpt_account_id(auth).unwrap_or_default();
    let url = format!("{CODEX_BACKEND}/codex/responses");
    let body = serde_json::json!({
        "model": model,
        "input": prompt,
        "store": false,
        "stream": false,
    });
    let mut builder = reqwest::blocking::Client::new()
        .post(&url)
        .bearer_auth(&tokens.access_token)
        .header("Content-Type", "application/json")
        .header("OpenAI-Beta", "responses=experimental")
        // Honest client id: we are grog, not the official Codex CLI.
        // The backend still sees the public Codex OAuth client id and
        // ChatGPT subscription token imported from ~/.codex/auth.json.
        .header("originator", "grog")
        .json(&body);
    if !account.is_empty() {
        builder = builder.header("ChatGPT-Account-Id", account);
    }
    let resp = builder.send().map_err(|e| ClientError::Transport(e.to_string()))?;
    let status = resp.status().as_u16();
    let json: serde_json::Value = resp.json().map_err(|e| ClientError::Transport(e.to_string()))?;
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
    let json: serde_json::Value = resp.json().map_err(|e| ClientError::Transport(e.to_string()))?;
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
}
