//! PATH + token checks for `grog doctor`.

use std::env;
use std::path::{Path, PathBuf};

use crate::ProviderId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub provider: ProviderId,
    pub ok: bool,
    pub detail: String,
}

pub fn doctor_checks() -> Vec<DoctorCheck> {
    vec![claude_check(), antigravity_check(), codex_check()]
}

pub fn format_doctor_report() -> String {
    let mut out = String::from("\nGrog providers\n");
    for check in doctor_checks() {
        let mark = if check.ok { "ok" } else { "missing" };
        out.push_str(&format!(
            "  {:<16} {mark:<8} {}\n",
            check.provider.as_str(),
            check.detail
        ));
    }
    out.push_str("\nPrivacy\n");
    out.push_str("  telemetry        off      default; remote settings cannot enable it\n");
    out.push_str("  mixpanel/sentry  off      no baked endpoints; Sentry DSN is runtime-only\n");
    out.push_str("  marketplace      empty    official xAI source is not auto-registered\n");
    out.push_str("  feedback         off      `/feedback` and trace cards are opt-in\n");
    out
}

fn claude_check() -> DoctorCheck {
    match which("claude") {
        Some(path) => DoctorCheck {
            provider: ProviderId::ClaudeBridge,
            ok: true,
            detail: format!(
                "{} (auth is Claude Code's ~/.claude, not grog)",
                path.display()
            ),
        },
        None => DoctorCheck {
            provider: ProviderId::ClaudeBridge,
            ok: false,
            detail: "claude not on PATH — install Claude Code and run `claude` once to log in"
                .into(),
        },
    }
}

fn antigravity_check() -> DoctorCheck {
    match which("agy") {
        Some(path) => DoctorCheck {
            provider: ProviderId::Antigravity,
            ok: true,
            detail: format!("{} (auth is agy's Google login, not grog)", path.display()),
        },
        None => DoctorCheck {
            provider: ProviderId::Antigravity,
            ok: false,
            detail: "agy not on PATH — install Google Antigravity / Cloud Code Assist".into(),
        },
    }
}

fn codex_check() -> DoctorCheck {
    let grog_home = xai_dirs::grok_home();
    let grog_path = grog_codex::grog_codex_auth_path(&grog_home);
    if grog_path.is_file() {
        return match grog_codex::load_auth(&grog_path) {
            Ok(_) => DoctorCheck {
                provider: ProviderId::Codex,
                ok: true,
                detail: format!("ChatGPT subscription tokens at {}", grog_path.display()),
            },
            Err(err) => DoctorCheck {
                provider: ProviderId::Codex,
                ok: false,
                detail: format!("{err}"),
            },
        };
    }
    if let Some(home) = xai_dirs::home_dir() {
        let cli = grog_codex::auth_json_path(&home);
        if cli.is_file() {
            return DoctorCheck {
                provider: ProviderId::Codex,
                ok: true,
                detail: format!(
                    "Codex CLI tokens at {} — run `grog login codex` to copy them into grog",
                    cli.display()
                ),
            };
        }
    }
    DoctorCheck {
        provider: ProviderId::Codex,
        ok: false,
        detail: "no ChatGPT Codex tokens — run `grog login codex` after `codex login`".into(),
    }
}

pub fn which(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(bin);
        candidate.is_file().then_some(candidate)
    })
}

pub fn bin_on_path(bin: &str) -> bool {
    which(bin).is_some()
}

/// Import `~/.codex/auth.json` into grog's auth dir. Claude/agy print how to log in.
pub fn login(provider: &str) -> Result<String, String> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "codex" | "chatgpt" | "openai-codex" => login_codex(),
        "claude" | "claude-bridge" => Ok(
            "Claude is a CLI bridge. Log in with Claude Code (`claude`), then pick a `claude-bridge/...` model in grog. Grog never holds an Anthropic token.".into(),
        ),
        "agy" | "antigravity" | "gemini" => Ok(
            "Antigravity is a CLI bridge. Log in with Google's `agy`, then pick an `antigravity/...` model in grog. Grog never holds a Google token.".into(),
        ),
        other => Err(format!(
            "unknown provider '{other}'. Try: grog login, grog login codex, grog login claude, grog login agy"
        )),
    }
}

fn login_codex() -> Result<String, String> {
    let user_home =
        xai_dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
    let cli = grog_codex::auth_json_path(&user_home);
    if !cli.is_file() {
        return Err(format!(
            "no Codex CLI tokens at {}. Run `codex login` first, then `grog login codex`.",
            cli.display()
        ));
    }
    let grog_home = xai_dirs::grok_home();
    let dest = grog_codex::import_codex_cli_auth(&cli, &grog_home).map_err(|e| e.to_string())?;
    Ok(format!(
        "Imported ChatGPT Codex subscription into {} (0600). Pick a `codex/...` model.",
        dest.display()
    ))
}

pub fn path_exists(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_always_reports_three_providers() {
        let checks = doctor_checks();
        assert_eq!(checks.len(), 3);
        assert!(
            checks
                .iter()
                .any(|c| c.provider == ProviderId::ClaudeBridge)
        );
        assert!(checks.iter().any(|c| c.provider == ProviderId::Antigravity));
        assert!(checks.iter().any(|c| c.provider == ProviderId::Codex));
        let text = format_doctor_report();
        assert!(text.contains("Grog providers"));
        assert!(text.contains("claude-bridge"));
        assert!(text.contains("Privacy"));
        assert!(text.contains("official xAI source is not auto-registered"));
    }

    #[test]
    fn login_rejects_unknown_and_explains_bridges() {
        assert!(login("nope").is_err());
        let claude = login("claude").unwrap();
        assert!(claude.contains("Claude Code"));
        let agy = login("agy").unwrap();
        assert!(agy.contains("agy"));
    }

    #[test]
    fn which_finds_sh_on_unix() {
        assert!(bin_on_path("sh"));
        assert!(!bin_on_path("definitely-not-a-grog-binary-xyz"));
    }
}
