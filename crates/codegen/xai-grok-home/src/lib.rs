//! Single source of truth for the grog/grok home directory.
//!
//! Resolution order: `$GROG_HOME`, then `$GROK_HOME`, then `<home>/.grog` if
//! that directory exists, then `<home>/.grok` if that directory exists,
//! otherwise `<home>/.grog` (created on first use).
//!
//! Shared by `xai-grok-config` and `xai-fast-worktree`.
//!
//! Which function to call:
//! - [`grok_home`]: the usual choice, a cached, created path to build on.
//! - [`user_grok_home`]: `None` instead of a cwd fallback when no home resolves.
//! - [`default_grok_home`]: the `<home>/.grog` default, ignoring env overrides.
//! - [`resolve_grok_home`]: a fresh, uncached resolve.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `<home>/.grog`, canonicalized via `dunce` (not `std::fs::canonicalize`,
/// which yields Windows `\\?\` verbatim paths).
fn grog_home_in(home: &Path) -> PathBuf {
    dunce::canonicalize(home)
        .unwrap_or_else(|_| home.to_path_buf())
        .join(".grog")
}

/// `<home>/.grok` (legacy Grok Build home).
fn grok_home_in(home: &Path) -> PathBuf {
    dunce::canonicalize(home)
        .unwrap_or_else(|_| home.to_path_buf())
        .join(".grok")
}

/// `$GROG_HOME` then `$GROK_HOME` verbatim when non-empty, else an existing
/// `<home>/.grog`, else an existing `<home>/.grok`, else `<home>/.grog`.
/// Env values are used as-is (not canonicalized) so they stay stable and
/// comparable: callers do literal prefix checks against them.
fn resolve_grok_home_from(
    grog_home_env: Option<&OsStr>,
    grok_home_env: Option<&OsStr>,
    os_home: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(env) = grog_home_env.filter(|env| !env.is_empty()) {
        return Some(PathBuf::from(env));
    }
    if let Some(env) = grok_home_env.filter(|env| !env.is_empty()) {
        return Some(PathBuf::from(env));
    }
    let os_home = os_home?;
    let grog = grog_home_in(os_home);
    if grog.is_dir() {
        return Some(grog);
    }
    let grok = grok_home_in(os_home);
    if grok.is_dir() {
        return Some(grok);
    }
    Some(grog)
}

/// Resolve the grog home from the environment (fresh, no cache); `None` if neither resolves.
pub fn resolve_grok_home() -> Option<PathBuf> {
    resolve_grok_home_from(
        std::env::var_os("GROG_HOME").as_deref(),
        std::env::var_os("GROK_HOME").as_deref(),
        dirs::home_dir().as_deref(),
    )
}

/// The default `<home>/.grog`, used when `$GROG_HOME` / `$GROK_HOME` are unset
/// and neither a `.grog` nor a `.grok` directory already exists.
pub fn default_grok_home() -> PathBuf {
    grog_home_in(&dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
}

/// The grog home, created if missing and cached for the process; falls back to
/// [`default_grok_home`] when neither env nor a home resolves.
pub fn grok_home() -> PathBuf {
    static GROK_HOME: OnceLock<PathBuf> = OnceLock::new();
    GROK_HOME
        .get_or_init(|| {
            let home = resolve_grok_home().unwrap_or_else(default_grok_home);
            if let Err(err) = std::fs::create_dir_all(&home) {
                tracing::warn!(path = %home.display(), %err, "failed to create grog home");
            }
            home
        })
        .clone()
}

/// Like [`grok_home`], but `None` when no home resolves (no cwd fallback).
pub fn user_grok_home() -> Option<PathBuf> {
    resolve_grok_home().is_some().then(grok_home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::ffi::OsString;

    #[test]
    fn grog_env_wins_over_grok_env_and_os_home() {
        let resolved = resolve_grok_home_from(
            Some(OsStr::new("/custom/grog")),
            Some(OsStr::new("/custom/grok")),
            Some(Path::new("/home/u")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/custom/grog")));
    }

    #[test]
    fn grok_env_wins_over_os_home_when_grog_env_unset() {
        let resolved = resolve_grok_home_from(
            None,
            Some(OsStr::new("/custom/home")),
            Some(Path::new("/home/u")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/custom/home")));
    }

    #[test]
    fn env_used_verbatim_even_when_it_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = resolve_grok_home_from(Some(tmp.path().as_os_str()), None, None);
        assert_eq!(resolved, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn empty_env_falls_through_to_grog_default() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved =
            resolve_grok_home_from(Some(&OsString::new()), Some(&OsString::new()), Some(tmp.path()));
        assert_eq!(
            resolved,
            Some(dunce::canonicalize(tmp.path()).unwrap().join(".grog"))
        );
    }

    #[test]
    fn existing_grok_home_is_used_when_grog_is_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let grok = tmp.path().join(".grok");
        std::fs::create_dir_all(&grok).unwrap();
        let resolved = resolve_grok_home_from(None, None, Some(tmp.path()));
        assert_eq!(resolved, Some(dunce::canonicalize(&grok).unwrap()));
    }

    #[test]
    fn existing_grog_home_wins_over_legacy_grok() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".grog")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".grok")).unwrap();
        let resolved = resolve_grok_home_from(None, None, Some(tmp.path()));
        assert_eq!(
            resolved,
            Some(dunce::canonicalize(tmp.path()).unwrap().join(".grog"))
        );
    }

    #[test]
    fn default_grok_home_has_no_verbatim_prefix() {
        let home = default_grok_home();
        assert!(!home.to_string_lossy().starts_with(r"\\?\"));
        assert!(home.ends_with(".grog"));
    }

    #[test]
    fn none_when_nothing_resolves() {
        assert_eq!(
            resolve_grok_home_from(None, None, None),
            None
        );
    }
}
