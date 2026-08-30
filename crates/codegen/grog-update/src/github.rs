//! Public GitHub Releases client for giiyms/grog-build.
//!
//! Uses the unauthenticated API. Optional `GITHUB_TOKEN` / `GH_TOKEN` from
//! the environment raises the rate limit; nothing is read from config.toml.

use crate::{PLATFORM, PRODUCT_CLI_NAME, UpdateError};
use serde::Deserialize;

pub const DEFAULT_REPO: &str = "giiyms/grog-build";
pub const ROLLING_TAG: &str = "grog-macos-aarch64";

#[derive(Debug, Clone, Deserialize)]
pub struct RollingRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckStatus {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub asset: String,
    pub digest: Option<String>,
    pub current_digest: Option<String>,
    pub tag: String,
}

pub fn github_api_base() -> String {
    std::env::var("GROG_GITHUB_API")
        .ok()
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://api.github.com".to_string())
}

pub fn update_repo() -> String {
    std::env::var("GROG_UPDATE_REPO")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

pub fn rolling_tag() -> String {
    std::env::var("GROG_UPDATE_TAG")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ROLLING_TAG.to_string())
}

fn version_tag(version: &str) -> String {
    let v = version.trim().trim_start_matches('v');
    format!("v{v}")
}

pub async fn fetch_release(
    client: &reqwest::Client,
    version: Option<&str>,
    api_base: Option<&str>,
) -> Result<RollingRelease, UpdateError> {
    let repo = update_repo();
    let tag = match version {
        Some(v) => version_tag(v),
        None => rolling_tag(),
    };
    let base = api_base
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(github_api_base);
    let url = format!("{base}/repos/{repo}/releases/tags/{tag}");
    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = github_token_from_env() {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(UpdateError::Github(tag, format!("HTTP {status}: {body}")));
    }
    serde_json::from_str(&body)
        .map_err(|e| UpdateError::Github(url, format!("invalid release JSON: {e}")))
}

pub async fn fetch_url_text(client: &reqwest::Client, url: &str) -> Result<String, UpdateError> {
    let resp = authorized_get(client, url).await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(UpdateError::message(format!(
            "download {url} failed: HTTP {status}"
        )));
    }
    Ok(text)
}

pub async fn fetch_url_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, UpdateError> {
    let resp = authorized_get(client, url).await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(UpdateError::message(format!(
            "download {url} failed: HTTP {status}"
        )));
    }
    Ok(resp.bytes().await?.to_vec())
}

async fn authorized_get(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, UpdateError> {
    let mut req = client.get(url);
    if let Some(token) = github_token_from_env() {
        req = req.bearer_auth(token);
    }
    Ok(req.send().await?)
}

/// Ambient GitHub token only. Never read from grog `config.toml`.
fn github_token_from_env() -> Option<String> {
    for key in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Ok(v) = std::env::var(key)
            && !v.is_empty()
        {
            return Some(v);
        }
    }
    None
}

/// Pick the versioned Darwin aarch64 grog binary, not the `.sha256` sidecar
/// and not official `grok-*` assets.
pub fn select_macos_aarch64_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let suffix = format!("-{PLATFORM}");
    let versioned = assets.iter().find(|a| {
        a.name.starts_with("grog-")
            && a.name.ends_with(&suffix)
            && !a.name.ends_with(".sha256")
            && crate::version_from_asset_name(&a.name).is_some()
    });
    if versioned.is_some() {
        return versioned;
    }
    assets
        .iter()
        .find(|a| a.name == format!("grog-{PLATFORM}") || a.name == PRODUCT_CLI_NAME)
}

/// `shasum -a 256` (`<hex>  <name>`) or a bare hex digest.
pub fn parse_sha256_text(text: &str) -> Option<String> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let token = line.split_whitespace().next()?;
    let hex = token.to_ascii_lowercase();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hex)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_versioned_grog_not_checksum_or_grok() {
        let assets = vec![
            ReleaseAsset {
                name: "grog-1.0.9-macos-aarch64.sha256".into(),
                browser_download_url: "https://example/sha".into(),
                size: 80,
            },
            ReleaseAsset {
                name: "grok-1.0.9-macos-aarch64".into(),
                browser_download_url: "https://example/grok".into(),
                size: 1,
            },
            ReleaseAsset {
                name: "grog-1.0.9-macos-aarch64".into(),
                browser_download_url: "https://example/grog".into(),
                size: 2,
            },
            ReleaseAsset {
                name: "grog-macos-aarch64".into(),
                browser_download_url: "https://example/stable".into(),
                size: 2,
            },
        ];
        let picked = select_macos_aarch64_asset(&assets).unwrap();
        assert_eq!(picked.name, "grog-1.0.9-macos-aarch64");
        assert!(picked.browser_download_url.contains("grog"));
        assert!(!picked.browser_download_url.contains("grok"));
    }

    #[test]
    fn parse_shasum_line() {
        assert_eq!(
            parse_sha256_text(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  grog-1.0.9-macos-aarch64\n"
            )
            .as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(parse_sha256_text("not-a-hash"), None);
    }
}
