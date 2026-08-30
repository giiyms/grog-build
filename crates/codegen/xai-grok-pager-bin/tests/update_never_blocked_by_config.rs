//! `grog update` must not be blocked by a corrupt config.toml, and must
//! never write official grok's `~/.grok` tree.

use std::io::{Read, Write};
use std::process::Command;

fn pager_binary() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("PAGER_BINARY") {
        return std::path::absolute(&p)
            .unwrap_or_else(|e| panic!("failed to absolutize PAGER_BINARY {p}: {e}"));
    }
    option_env!("CARGO_BIN_EXE_grog")
        .or(option_env!("CARGO_BIN_EXE_xai-grok-pager"))
        .map(std::path::PathBuf::from)
        .expect("PAGER_BINARY is unset and this build is not `cargo test`")
}

/// Serve GitHub-shaped release JSON plus the binary (no checksum: optional).
fn spawn_github_mock(payload: &'static [u8]) -> (std::net::TcpListener, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let serving = listener.try_clone().unwrap();
    let base_for_json = base.clone();
    std::thread::spawn(move || {
        let release = serde_json::json!({
            "tag_name": "grog-macos-aarch64",
            "name": "grog 1.0.9 (macOS aarch64, rolling)",
            "assets": [{
                "name": "grog-1.0.9-macos-aarch64",
                "browser_download_url": format!("{base_for_json}/dl/grog"),
                "size": payload.len()
            }]
        })
        .to_string();
        for stream in serving.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let (status, body, content_type): (&str, Vec<u8>, &str) = if req.contains(" /dl/grog ")
            {
                ("200 OK", payload.to_vec(), "application/octet-stream")
            } else if req.contains("/releases/tags/") {
                ("200 OK", release.as_bytes().to_vec(), "application/json")
            } else {
                ("404 Not Found", b"missing".to_vec(), "text/plain")
            };
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = stream.write_all(&body);
        }
    });
    (listener, base)
}

fn run_update(
    api: &str,
    home: &std::path::Path,
    config_toml: &str,
    extra_args: &[&str],
) -> std::process::Output {
    std::fs::write(home.join("config.toml"), config_toml).unwrap();
    let grog_home = home.join(".grog");
    std::fs::create_dir_all(&grog_home).unwrap();
    let grok_home = home.join(".grok");
    std::fs::create_dir_all(&grok_home).unwrap();
    std::fs::write(grok_home.join("official-marker"), b"do-not-touch").unwrap();
    Command::new(pager_binary())
        .arg("update")
        .args(extra_args)
        .env_clear()
        .env("HOME", home)
        .env("GROG_HOME", &grog_home)
        .env("GROK_HOME", &grok_home)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("GROG_GITHUB_API", api)
        .env("GROG_UPDATE_SKIP_PLATFORM", "1")
        .output()
        .expect("spawn grog update")
}

#[test]
fn corrupt_config_never_changes_update_outcome() {
    let payload: &'static [u8] = b"grog-test-payload-1.0.9";
    let (_listener, api) = spawn_github_mock(payload);

    let home_ok = tempfile::tempdir().unwrap();
    let check = run_update(
        api.as_str(),
        home_ok.path(),
        "[cli]\n",
        &["--check", "--json"],
    );
    assert!(
        check.status.success(),
        "grog update --check --json must exit 0\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap_or_else(|e| {
        panic!(
            "update --check --json must emit JSON: {e}\nstdout:\n{}",
            String::from_utf8_lossy(&check.stdout)
        )
    });
    assert_eq!(status["name"], "grog");
    assert_ne!(status["name"], "grok");
    assert_eq!(status["latestVersion"], "1.0.9");
    assert!(status["currentVersion"].as_str().is_some());

    let valid = run_update(api.as_str(), home_ok.path(), "[cli]\n", &[]);
    assert!(
        valid.status.success(),
        "healthy grog update must exit 0\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&valid.stdout),
        String::from_utf8_lossy(&valid.stderr)
    );
    let stdout = String::from_utf8_lossy(&valid.stdout);
    assert!(
        stdout.contains("grog"),
        "install copy must name grog: {stdout}"
    );
    assert!(
        !stdout.to_ascii_lowercase().contains("grok"),
        "install copy must not say grok: {stdout}"
    );

    let home_bad = tempfile::tempdir().unwrap();
    let corrupt = run_update(
        api.as_str(),
        home_bad.path(),
        "this is not toml {{{[[[",
        &[],
    );
    assert!(
        corrupt.status.success(),
        "a corrupt config.toml must not block grog update\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&corrupt.stdout),
        String::from_utf8_lossy(&corrupt.stderr)
    );

    let installed = home_ok
        .path()
        .join(".grog/downloads/grog-1.0.9-macos-aarch64");
    assert_eq!(std::fs::read(&installed).unwrap(), payload);
    assert!(!home_ok.path().join(".grog/bin/grok").exists());
    assert_eq!(
        std::fs::read(home_ok.path().join(".grok/official-marker")).unwrap(),
        b"do-not-touch"
    );
    assert_eq!(
        std::fs::read(home_bad.path().join(".grok/official-marker")).unwrap(),
        b"do-not-touch"
    );
}
