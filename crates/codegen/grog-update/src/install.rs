//! Install layout under `~/.grog` (never `~/.grok`).
//!
//! ```text
//! ~/.grog/downloads/grog-<ver>-macos-aarch64
//! ~/.grog/bin/grog -> ../downloads/grog-<ver>-macos-aarch64
//! ~/.local/bin/grog -> ~/.grog/bin/grog
//! ```

use crate::UpdateError;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GrogLayout {
    pub home: PathBuf,
    pub download_path: PathBuf,
    pub bin_link: PathBuf,
    pub user_link: PathBuf,
    relative_download: PathBuf,
}

#[derive(Debug, Clone)]
pub struct InstalledPaths {
    pub download_path: PathBuf,
    pub bin_link: PathBuf,
    pub user_link: Option<PathBuf>,
}

/// `$GROG_HOME` when it is not a grok tree, else `<os-home>/.grog`.
/// Never `$GROK_HOME`, never `<home>/.grok`.
pub fn grog_install_home() -> Result<PathBuf, UpdateError> {
    grog_install_home_from(
        std::env::var_os("GROG_HOME").as_deref(),
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .as_path(),
    )
}

pub fn grog_install_home_from(
    grog_home_env: Option<&OsStr>,
    os_home: &Path,
) -> Result<PathBuf, UpdateError> {
    if let Some(env) = grog_home_env.filter(|e| !e.is_empty()) {
        let path = PathBuf::from(env);
        assert_not_grok_tree(&path)?;
        return Ok(path);
    }
    let grog = os_home.join(".grog");
    assert_not_grok_tree(&grog)?;
    Ok(grog)
}

pub fn user_local_bin_grog(os_home: &Path) -> PathBuf {
    os_home.join(".local").join("bin").join("grog")
}

pub fn assert_not_grok_tree(path: &Path) -> Result<(), UpdateError> {
    for c in path.components() {
        if c.as_os_str() == ".grok" {
            return Err(UpdateError::GrokHomeForbidden);
        }
    }
    Ok(())
}

pub fn layout_for(
    home: &Path,
    asset_name: &str,
    user_home: &Path,
) -> Result<GrogLayout, UpdateError> {
    assert_not_grok_tree(home)?;
    if asset_name.contains("grok") && !asset_name.starts_with("grog-") {
        return Err(UpdateError::message(
            "refusing to install an asset whose name is not grog-*",
        ));
    }
    let download_path = home.join("downloads").join(asset_name);
    assert_not_grok_tree(&download_path)?;
    let bin_link = home.join("bin").join("grog");
    assert_not_grok_tree(&bin_link)?;
    let user_link = user_local_bin_grog(user_home);
    Ok(GrogLayout {
        home: home.to_path_buf(),
        download_path,
        bin_link,
        user_link,
        relative_download: PathBuf::from("..").join("downloads").join(asset_name),
    })
}

pub fn install_file(layout: &GrogLayout, bytes: &[u8]) -> Result<InstalledPaths, UpdateError> {
    assert_not_grok_tree(&layout.home)?;
    assert_not_grok_tree(&layout.download_path)?;
    assert_not_grok_tree(&layout.bin_link)?;

    std::fs::create_dir_all(layout.download_path.parent().unwrap())?;
    std::fs::create_dir_all(layout.bin_link.parent().unwrap())?;

    let tmp = layout.download_path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
    }
    std::fs::rename(&tmp, &layout.download_path)?;

    replace_symlink(&layout.bin_link, &layout.relative_download)?;

    let user_link = match replace_user_link(&layout.user_link, &layout.bin_link) {
        Ok(true) => Some(layout.user_link.clone()),
        Ok(false) => None,
        Err(e) => {
            tracing_warn(format!(
                "could not link {}: {e}",
                layout.user_link.display()
            ));
            None
        }
    };

    Ok(InstalledPaths {
        download_path: layout.download_path.clone(),
        bin_link: layout.bin_link.clone(),
        user_link,
    })
}

fn tracing_warn(msg: String) {
    eprintln!("grog update: {msg}");
}

fn replace_symlink(link: &Path, target: &Path) -> Result<(), UpdateError> {
    assert_not_grok_tree(link)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let tmp = link.with_extension("tmp-link");
        let _ = std::fs::remove_file(&tmp);
        if let Ok(meta) = std::fs::symlink_metadata(link) {
            if meta.file_type().is_dir() {
                return Err(UpdateError::message(format!(
                    "{} is a directory; refusing to replace",
                    link.display()
                )));
            }
            std::fs::remove_file(link)?;
        }
        symlink(target, &tmp)?;
        std::fs::rename(&tmp, link)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (link, target);
        Err(UpdateError::message(
            "grog update only installs on Unix (Darwin aarch64 ship path)",
        ))
    }
}

/// Link `~/.local/bin/grog` at `bin_link` unless that path is a non-grog file
/// we should not clobber. Never writes `grok`.
fn replace_user_link(user_link: &Path, bin_link: &Path) -> Result<bool, UpdateError> {
    if user_link.file_name().is_some_and(|n| n == "grok") {
        return Err(UpdateError::message("refusing to write ~/.local/bin/grok"));
    }
    if let Some(parent) = user_link.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Ok(meta) = std::fs::symlink_metadata(user_link)
        && meta.file_type().is_dir()
    {
        return Err(UpdateError::message(format!(
            "{} is a directory",
            user_link.display()
        )));
    }
    replace_symlink(user_link, bin_link)?;
    Ok(true)
}

pub fn digest_file(path: &Path) -> Result<String, UpdateError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn current_binary_digest(exe: &Path) -> Result<String, UpdateError> {
    digest_file(exe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn default_home_is_grog_even_when_grok_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".grok")).unwrap();
        let home = grog_install_home_from(None, tmp.path()).unwrap();
        assert_eq!(home, tmp.path().join(".grog"));
        assert!(!home.ends_with(".grok"));
    }

    #[test]
    fn grog_home_env_rejected_when_it_is_grok() {
        let err = grog_install_home_from(Some(OsStr::new("/Users/d/.grok")), Path::new("/Users/d"))
            .unwrap_err();
        assert!(matches!(err, UpdateError::GrokHomeForbidden));
    }

    #[test]
    fn grog_home_env_accepted() {
        let home =
            grog_install_home_from(Some(OsStr::new("/tmp/custom-grog")), Path::new("/Users/d"))
                .unwrap();
        assert_eq!(home, PathBuf::from("/tmp/custom-grog"));
    }

    #[test]
    fn empty_grog_home_falls_through() {
        let tmp = tempfile::tempdir().unwrap();
        let home = grog_install_home_from(Some(&OsString::new()), tmp.path()).unwrap();
        assert_eq!(home, tmp.path().join(".grog"));
    }

    #[test]
    fn assert_not_grok_tree_catches_nested() {
        let err = assert_not_grok_tree(Path::new("/Users/d/.grok/downloads/grog")).unwrap_err();
        assert!(matches!(err, UpdateError::GrokHomeForbidden));
    }

    #[test]
    fn install_writes_grog_layout_not_grok() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join(".grog");
        let layout = layout_for(&home, "grog-1.0.9-macos-aarch64", tmp.path()).unwrap();
        let installed = install_file(&layout, b"#!/bin/sh\necho grog 1.0.9\n").unwrap();
        assert_eq!(
            installed.download_path,
            home.join("downloads").join("grog-1.0.9-macos-aarch64")
        );
        assert!(installed.download_path.exists());
        let bin = home.join("bin").join("grog");
        assert!(bin.exists());
        let target = std::fs::read_link(&bin).unwrap();
        assert_eq!(
            target,
            PathBuf::from("../downloads/grog-1.0.9-macos-aarch64")
        );
        assert_eq!(
            installed.user_link.as_deref(),
            Some(tmp.path().join(".local/bin/grog").as_path())
        );
        assert!(tmp.path().join(".local/bin/grog").exists());
        assert!(!tmp.path().join(".grok").exists());
        assert!(!home.join("bin").join("grok").exists());
        assert!(!tmp.path().join(".local/bin/grok").exists());
    }

    #[test]
    fn user_local_bin_is_grog() {
        let p = user_local_bin_grog(Path::new("/Users/d"));
        assert_eq!(p, PathBuf::from("/Users/d/.local/bin/grog"));
        assert!(!p.as_os_str().to_string_lossy().contains("grok"));
    }
}
