//! Persist Codex OAuth under grog home. Prefer importing `~/.codex/auth.json`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::auth::{parse_auth_json, CodexAuth, CodexAuthError};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Auth(#[from] CodexAuthError),
    #[error("read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("no ChatGPT Codex tokens at {path}")]
    Missing { path: PathBuf },
}

pub fn grog_codex_auth_path(grog_home: &Path) -> PathBuf {
    grog_home.join("auth").join("codex.json")
}

pub fn load_auth(path: &Path) -> Result<CodexAuth, StoreError> {
    let json = fs::read_to_string(path).map_err(|source| StoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(parse_auth_json(&json)?)
}

pub fn save_auth(path: &Path, auth: &CodexAuth) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| StoreError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    let json = serde_json::to_string_pretty(auth).expect("CodexAuth serializes");
    fs::write(path, json).map_err(|source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Copy a Codex CLI `auth.json` into grog after verifying it is a subscription.
pub fn import_codex_cli_auth(
    codex_auth_json: &Path,
    grog_home: &Path,
) -> Result<PathBuf, StoreError> {
    let auth = load_auth(codex_auth_json)?;
    let dest = grog_codex_auth_path(grog_home);
    save_auth(&dest, &auth)?;
    Ok(dest)
}

pub fn load_grog_or_import(grog_home: &Path, user_home: &Path) -> Result<CodexAuth, StoreError> {
    let grog_path = grog_codex_auth_path(grog_home);
    if grog_path.is_file() {
        return load_auth(&grog_path);
    }
    let cli_path = crate::auth::auth_json_path(user_home);
    if cli_path.is_file() {
        let dest = import_codex_cli_auth(&cli_path, grog_home)?;
        return load_auth(&dest);
    }
    Err(StoreError::Missing { path: grog_path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn import_rejects_api_key_only_and_copies_subscription() {
        let dir = tempdir().unwrap();
        let cli = dir.path().join("codex-auth.json");
        fs::write(&cli, r#"{"OPENAI_API_KEY":"sk-test"}"#).unwrap();
        let grog = dir.path().join("grog");
        assert!(import_codex_cli_auth(&cli, &grog).is_err());

        fs::write(
            &cli,
            r#"{"tokens":{"access_token":"tok","refresh_token":"ref","account_id":"acct"}}"#,
        )
        .unwrap();
        let dest = import_codex_cli_auth(&cli, &grog).unwrap();
        assert!(dest.ends_with("auth/codex.json"));
        let loaded = load_auth(&dest).unwrap();
        assert_eq!(loaded.tokens.unwrap().access_token, "tok");
    }
}
