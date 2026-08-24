//! Codex OAuth token file. Compatible with Codex CLI `auth.json`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const AUTH_ISSUER: &str = "https://auth.openai.com";
pub const DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";
/// Public Codex CLI OAuth client id.
pub const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const CODEX_BACKEND: &str = "https://chatgpt.com/backend-api";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexAuth {
    #[serde(default)]
    pub tokens: Option<CodexTokens>,
    #[serde(default)]
    pub last_refresh: Option<String>,
    /// Present when the user used an API key instead of ChatGPT OAuth.
    /// Grog's subscription provider ignores this.
    #[serde(default, rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexTokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CodexAuthError {
    #[error(
        "auth.json has no ChatGPT OAuth tokens (OPENAI_API_KEY-only files are not a subscription)"
    )]
    NotSubscription,
    #[error("invalid auth.json: {0}")]
    Parse(#[from] serde_json::Error),
}

pub fn parse_auth_json(json: &str) -> Result<CodexAuth, CodexAuthError> {
    let auth: CodexAuth = serde_json::from_str(json)?;
    if auth
        .tokens
        .as_ref()
        .map(|t| t.access_token.is_empty())
        .unwrap_or(true)
    {
        return Err(CodexAuthError::NotSubscription);
    }
    Ok(auth)
}

/// `~/.codex/auth.json` — reuse Codex CLI login when present.
pub fn auth_json_path(home: &Path) -> PathBuf {
    home.join(".codex").join("auth.json")
}

/// Read `chatgpt_account_id` from the access token's
/// `https://api.openai.com/auth` JWT claim when the file omitted `account_id`.
pub fn chatgpt_account_id(auth: &CodexAuth) -> Option<String> {
    let tokens = auth.tokens.as_ref()?;
    if let Some(id) = tokens.account_id.as_ref().filter(|s| !s.is_empty()) {
        return Some(id.clone());
    }
    jwt_claim_account_id(&tokens.access_token)
}

fn jwt_claim_account_id(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let padded = match payload.len() % 4 {
        0 => payload.to_string(),
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let bytes = b64url_decode(&padded)?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("https://api.openai.com/auth")
        .and_then(|a| a.get("chatgpt_account_id"))
        .and_then(|id| id.as_str())
        .map(str::to_string)
}

fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut t = s.replace('-', "+").replace('_', "/");
    while t.len() % 4 != 0 {
        t.push('=');
    }
    b64_std(&t)
}

fn b64_std(s: &str) -> Option<Vec<u8>> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [0xffu8; 256];
    for (i, b) in T.iter().enumerate() {
        table[*b as usize] = i as u8;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut n = 0;
    for &c in bytes {
        if c == b'=' {
            break;
        }
        let v = table[c as usize];
        if v == 0xff {
            return None;
        }
        buf = (buf << 6) | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((buf >> n) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_api_key_only_file() {
        let err = parse_auth_json(r#"{"OPENAI_API_KEY":"sk-test"}"#).unwrap_err();
        assert!(matches!(err, CodexAuthError::NotSubscription));
    }

    #[test]
    fn parses_cli_auth_json() {
        let auth = parse_auth_json(
            r#"{"tokens":{"access_token":"tok","refresh_token":"ref","account_id":"acct-1"}}"#,
        )
        .unwrap();
        assert_eq!(chatgpt_account_id(&auth).as_deref(), Some("acct-1"));
        assert_eq!(auth.tokens.unwrap().refresh_token.as_deref(), Some("ref"));
    }

    #[test]
    fn account_id_from_jwt_claim() {
        // header.payload.sig — payload is {"https://api.openai.com/auth":{"chatgpt_account_id":"acc-jwt"}}
        let payload =
            base64url(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acc-jwt"}}"#);
        let jwt = format!("e30.{payload}.sig");
        let auth = CodexAuth {
            tokens: Some(CodexTokens {
                access_token: jwt,
                refresh_token: None,
                id_token: None,
                account_id: None,
            }),
            last_refresh: None,
            openai_api_key: None,
        };
        assert_eq!(chatgpt_account_id(&auth).as_deref(), Some("acc-jwt"));
    }

    fn base64url(bytes: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for chunk in bytes.chunks(3) {
            let a = chunk[0] as u32;
            let b = chunk.get(1).copied().unwrap_or(0) as u32;
            let c = chunk.get(2).copied().unwrap_or(0) as u32;
            let n = (a << 16) | (b << 8) | c;
            s.push(T[((n >> 18) & 63) as usize] as char);
            s.push(T[((n >> 12) & 63) as usize] as char);
            if chunk.len() > 1 {
                s.push(T[((n >> 6) & 63) as usize] as char);
            }
            if chunk.len() > 2 {
                s.push(T[(n & 63) as usize] as char);
            }
        }
        s.replace('+', "-").replace('/', "_")
    }

    #[test]
    fn default_auth_path_is_codex_cli_home() {
        assert_eq!(
            auth_json_path(Path::new("/home/me")),
            PathBuf::from("/home/me/.codex/auth.json")
        );
    }
}
