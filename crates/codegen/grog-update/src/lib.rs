//! Install grog from this fork's GitHub Releases.
//!
//! Ship path is the GitHub-hosted Darwin aarch64 binary
//! (`giiyms/grog-build`, rolling tag [`ROLLING_TAG`]). This crate never
//! talks to x.ai/cli, never writes `~/.grok`, and never installs a binary
//! named `grok`.

mod github;
mod install;

pub use github::{
    CheckStatus, DEFAULT_REPO, ROLLING_TAG, ReleaseAsset, RollingRelease, github_api_base,
    parse_sha256_text, select_macos_aarch64_asset,
};
pub use install::{
    GrogLayout, InstalledPaths, assert_not_grok_tree, grog_install_home, grog_install_home_from,
    install_file, layout_for, user_local_bin_grog,
};

use github::{fetch_release, fetch_url_bytes, fetch_url_text};
use install::{current_binary_digest, digest_file};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Artifact platform this fork publishes.
pub const PLATFORM: &str = "macos-aarch64";

/// Public CLI name. The downloaded file must identify as this, not `grok`.
pub const PRODUCT_CLI_NAME: &str = "grog";

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{0}")]
    Message(String),
    #[error("grog update refuses to write under ~/.grok (official grok lives there)")]
    GrokHomeForbidden,
    #[error("this grog ship path is Darwin aarch64 only (host is {os}-{arch})")]
    UnsupportedPlatform {
        os: &'static str,
        arch: &'static str,
    },
    #[error("GitHub release {0}: {1}")]
    Github(String, String),
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

impl UpdateError {
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpdateOptions {
    /// Re-download even when the installed digest already matches.
    pub force: bool,
    /// Resolve the rolling (or versioned) release without installing.
    pub check_only: bool,
    /// Install a numbered release (`1.0.9` or `v1.0.9`) instead of the rolling tag.
    pub version: Option<String>,
    /// Tests and Linux CI: skip the Darwin aarch64 host gate.
    pub skip_platform_check: bool,
    /// Override `$GROG_HOME` / `~/.grog` (tests).
    pub install_home: Option<PathBuf>,
    /// Override the OS home used for `~/.local/bin/grog` (tests).
    pub user_home: Option<PathBuf>,
    /// Override GitHub API origin (`GROG_GITHUB_API`).
    pub api_base: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    AlreadyCurrent {
        version: String,
        digest: String,
        path: PathBuf,
    },
    Installed {
        version: String,
        digest: String,
        path: PathBuf,
        bin_link: PathBuf,
        user_link: Option<PathBuf>,
    },
    Available {
        status: CheckStatus,
    },
}

impl UpdateOutcome {
    pub fn cli_message(&self) -> String {
        match self {
            Self::AlreadyCurrent { version, .. } => {
                format!("grog is already up to date ({version}).")
            }
            Self::Installed {
                version,
                path,
                bin_link,
                user_link,
                ..
            } => {
                let mut msg = format!(
                    "Installed grog {version} to {}\nLinked {}",
                    path.display(),
                    bin_link.display()
                );
                if let Some(user) = user_link {
                    msg.push_str(&format!(" and {}", user.display()));
                }
                msg.push('.');
                msg
            }
            Self::Available { status } => {
                if status.update_available {
                    format!(
                        "grog {} is available (installed {}).",
                        status.latest_version, status.current_version
                    )
                } else {
                    format!("grog is already up to date ({}).", status.current_version)
                }
            }
        }
    }
}

/// Running grog version (`grog --version` without the commit), from
/// `xai-grok-version` (lockstepped with the pager-bin crate).
pub fn running_version() -> String {
    xai_grok_version::installed()
}

pub fn host_is_macos_aarch64() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

pub fn skip_platform_from_env() -> bool {
    std::env::var_os("GROG_UPDATE_SKIP_PLATFORM").is_some_and(|v| {
        let s = v.to_string_lossy();
        !s.is_empty() && s != "0" && !s.eq_ignore_ascii_case("false")
    })
}

/// Fetch + optionally install the Darwin aarch64 grog from GitHub Releases.
pub async fn run_update(opts: &UpdateOptions) -> Result<UpdateOutcome, UpdateError> {
    if !opts.skip_platform_check && !skip_platform_from_env() && !host_is_macos_aarch64() {
        return Err(UpdateError::UnsupportedPlatform {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        });
    }

    let home = match &opts.install_home {
        Some(h) => {
            assert_not_grok_tree(h)?;
            h.clone()
        }
        None => grog_install_home()?,
    };
    let user_home = opts
        .user_home
        .clone()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    assert_not_grok_tree(&home)?;

    let client = http_client()?;
    let release = fetch_release(&client, opts.version.as_deref(), opts.api_base.as_deref()).await?;
    let asset = select_macos_aarch64_asset(&release.assets).ok_or_else(|| {
        UpdateError::Github(
            release.tag_name.clone(),
            format!(
                "no grog-*-{PLATFORM} asset (not .sha256) on this release; \
                 grog update expects the macos-14 Darwin aarch64 build"
            ),
        )
    })?;

    let version = version_from_asset_name(&asset.name)
        .or_else(|| version_from_release_title(&release.name))
        .unwrap_or_else(|| release.tag_name.trim_start_matches('v').to_string());

    let expected_digest = match sibling_sha256(&release.assets, &asset.name) {
        Some(sha_asset) => {
            let text = fetch_url_text(&client, &sha_asset.browser_download_url).await?;
            Some(parse_sha256_text(&text).ok_or_else(|| {
                UpdateError::message(format!("could not parse checksum file {}", sha_asset.name))
            })?)
        }
        None => None,
    };

    let layout = layout_for(&home, &asset.name, &user_home)?;
    let current = installed_or_running_digest(&layout.bin_link)?;
    let already = match (&expected_digest, &current) {
        (Some(expected), Some(got)) => expected == got,
        _ => false,
    };

    let status = CheckStatus {
        name: PRODUCT_CLI_NAME.to_string(),
        current_version: running_version(),
        latest_version: version.clone(),
        update_available: !already,
        asset: asset.name.clone(),
        digest: expected_digest.clone(),
        current_digest: current.clone(),
        tag: release.tag_name.clone(),
    };

    if opts.check_only {
        return Ok(UpdateOutcome::Available { status });
    }
    if already && !opts.force {
        return Ok(UpdateOutcome::AlreadyCurrent {
            version,
            digest: expected_digest.unwrap_or_default(),
            path: layout.download_path.clone(),
        });
    }

    let bytes = fetch_url_bytes(&client, &asset.browser_download_url).await?;
    let actual = sha256_hex(&bytes);
    if let Some(expected) = expected_digest.as_ref()
        && actual != *expected
    {
        return Err(UpdateError::ChecksumMismatch {
            expected: expected.clone(),
            actual,
        });
    }

    let installed = install_file(&layout, &bytes)?;
    smoke_test(&installed.download_path)?;

    Ok(UpdateOutcome::Installed {
        version,
        digest: actual,
        path: installed.download_path,
        bin_link: installed.bin_link,
        user_link: installed.user_link,
    })
}

fn http_client() -> Result<reqwest::Client, UpdateError> {
    xai_grok_extra_ca::build_reqwest_client(|builder| {
        builder
            .user_agent(format!(
                "grog ({PRODUCT_CLI_NAME}; +https://github.com/{DEFAULT_REPO})"
            ))
            .redirect(reqwest::redirect::Policy::limited(10))
    })
    .map_err(UpdateError::from)
}

fn sibling_sha256<'a>(assets: &'a [ReleaseAsset], binary_name: &str) -> Option<&'a ReleaseAsset> {
    let want = format!("{binary_name}.sha256");
    assets.iter().find(|a| a.name == want)
}

pub fn version_from_asset_name(name: &str) -> Option<String> {
    let prefix = "grog-";
    let suffix = format!("-{PLATFORM}");
    let rest = name.strip_prefix(prefix)?;
    let ver = rest.strip_suffix(&suffix)?;
    if ver.is_empty() || ver == "macos" {
        return None;
    }
    Some(ver.to_string())
}

fn version_from_release_title(title: &str) -> Option<String> {
    // "grog 1.0.9 (macOS aarch64)" or "grog 1.0.9"
    let rest = title.trim().strip_prefix("grog ")?;
    rest.split_whitespace()
        .next()
        .filter(|s| !s.is_empty() && s.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
}

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn installed_or_running_digest(bin_link: &Path) -> Result<Option<String>, UpdateError> {
    if bin_link.exists() {
        return Ok(Some(digest_file(bin_link)?));
    }
    match std::env::current_exe() {
        Ok(exe) => current_binary_digest(&exe).map(Some),
        Err(_) => Ok(None),
    }
}

fn smoke_test(path: &Path) -> Result<(), UpdateError> {
    if skip_platform_from_env() || !host_is_macos_aarch64() {
        return Ok(());
    }
    let output = std::process::Command::new(path)
        .arg("--version")
        .output()
        .map_err(|e| UpdateError::message(format!("could not run downloaded grog: {e}")))?;
    if !output.status.success() {
        return Err(UpdateError::message(format!(
            "downloaded grog --version failed ({})",
            output.status
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next().unwrap_or("").trim();
    if !first.starts_with("grog ") {
        return Err(UpdateError::message(format!(
            "refusing to install a binary that is not grog (got {first:?})"
        )));
    }
    Ok(())
}

pub fn check_status_json(status: &CheckStatus) -> Result<String, serde_json::Error> {
    serde_json::to_string(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_name_version_parses() {
        assert_eq!(
            version_from_asset_name("grog-1.0.9-macos-aarch64").as_deref(),
            Some("1.0.9")
        );
        assert_eq!(version_from_asset_name("grog-macos-aarch64"), None);
        assert_eq!(
            version_from_asset_name("grog-1.0.9-macos-aarch64.sha256"),
            None
        );
    }

    #[test]
    fn release_title_version_parses() {
        assert_eq!(
            version_from_release_title("grog 1.0.9 (macOS aarch64, rolling)").as_deref(),
            Some("1.0.9")
        );
        assert_eq!(version_from_release_title("not grog"), None);
    }

    #[test]
    fn cli_messages_name_grog_not_grok() {
        let already = UpdateOutcome::AlreadyCurrent {
            version: "1.0.9".into(),
            digest: "abc".into(),
            path: PathBuf::from("/tmp/grog"),
        };
        let msg = already.cli_message();
        assert!(msg.contains("grog"));
        assert!(!msg.to_ascii_lowercase().contains("grok"));
        assert!(msg.contains("already up to date"));
    }

    #[test]
    fn product_name_is_grog() {
        assert_eq!(PRODUCT_CLI_NAME, "grog");
        assert_ne!(PRODUCT_CLI_NAME, "grok");
        assert_eq!(ROLLING_TAG, "grog-macos-aarch64");
        assert_eq!(DEFAULT_REPO, "giiyms/grog-build");
    }
}
