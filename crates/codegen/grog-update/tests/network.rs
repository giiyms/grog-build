//! HTTP install against a mock GitHub Releases API.

use grog_update::{UpdateOptions, UpdateOutcome, parse_sha256_text, run_update};
use sha2::{Digest, Sha256};

fn sha(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

#[tokio::test]
async fn downloads_verifies_checksum_and_installs_into_grog_home() {
    let tmp = tempfile::tempdir().unwrap();
    let grog_home = tmp.path().join(".grog");
    let grok_home = tmp.path().join(".grok");
    std::fs::create_dir_all(&grok_home).unwrap();
    std::fs::write(grok_home.join("bin-grok-marker"), b"official grok stays").unwrap();

    let payload = b"grog-fake-binary-v1.0.9";
    let digest = sha(payload);
    let mut server = mockito::Server::new_async().await;
    let api = server.url();

    let release = serde_json::json!({
        "tag_name": "grog-macos-aarch64",
        "name": "grog 1.0.9 (macOS aarch64, rolling)",
        "assets": [
            {
                "name": "grog-1.0.9-macos-aarch64",
                "browser_download_url": format!("{api}/dl/grog"),
                "size": payload.len()
            },
            {
                "name": "grog-1.0.9-macos-aarch64.sha256",
                "browser_download_url": format!("{api}/dl/grog.sha256"),
                "size": 80
            },
            {
                "name": "grok-1.0.9-macos-aarch64",
                "browser_download_url": format!("{api}/dl/grok"),
                "size": 1
            }
        ]
    });

    let _rel = server
        .mock(
            "GET",
            "/repos/giiyms/grog-build/releases/tags/grog-macos-aarch64",
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(release.to_string())
        .create_async()
        .await;
    let _bin = server
        .mock("GET", "/dl/grog")
        .with_status(200)
        .with_body(payload.as_slice())
        .create_async()
        .await;
    let _sha = server
        .mock("GET", "/dl/grog.sha256")
        .with_status(200)
        .with_body(format!("{digest}  grog-1.0.9-macos-aarch64\n"))
        .create_async()
        .await;
    let grok_dl = server
        .mock("GET", "/dl/grok")
        .with_status(200)
        .with_body("nope")
        .expect(0)
        .create_async()
        .await;

    let opts = UpdateOptions {
        skip_platform_check: true,
        install_home: Some(grog_home.clone()),
        user_home: Some(tmp.path().to_path_buf()),
        api_base: Some(api.clone()),
        ..UpdateOptions::default()
    };
    let outcome = run_update(&opts).await.expect("install");
    match outcome {
        UpdateOutcome::Installed {
            version,
            path,
            bin_link,
            user_link,
            ..
        } => {
            assert_eq!(version, "1.0.9");
            assert_eq!(path, grog_home.join("downloads/grog-1.0.9-macos-aarch64"));
            assert_eq!(std::fs::read(&path).unwrap(), payload);
            assert_eq!(bin_link, grog_home.join("bin/grog"));
            assert_eq!(
                std::fs::read_link(&bin_link).unwrap(),
                std::path::PathBuf::from("../downloads/grog-1.0.9-macos-aarch64")
            );
            assert_eq!(
                user_link.as_deref(),
                Some(tmp.path().join(".local/bin/grog").as_path())
            );
        }
        other => panic!("expected Installed, got {other:?}"),
    }

    grok_dl.assert_async().await;
    assert_eq!(
        std::fs::read(grok_home.join("bin-grok-marker")).unwrap(),
        b"official grok stays"
    );
    assert!(!grog_home.join("bin/grok").exists());
    assert!(!tmp.path().join(".local/bin/grok").exists());

    let again = run_update(&opts).await.expect("second update");
    assert!(
        matches!(again, UpdateOutcome::AlreadyCurrent { ref version, .. } if version == "1.0.9"),
        "second update must no-op, got {again:?}"
    );
}

#[tokio::test]
async fn check_only_does_not_write() {
    let tmp = tempfile::tempdir().unwrap();
    let grog_home = tmp.path().join(".grog");
    let payload = b"x";
    let digest = sha(payload);
    let mut server = mockito::Server::new_async().await;
    let api = server.url();
    let release = serde_json::json!({
        "tag_name": "grog-macos-aarch64",
        "name": "grog 1.0.9",
        "assets": [{
            "name": "grog-1.0.9-macos-aarch64",
            "browser_download_url": format!("{api}/dl/grog"),
            "size": 1
        }, {
            "name": "grog-1.0.9-macos-aarch64.sha256",
            "browser_download_url": format!("{api}/dl/sha"),
            "size": 80
        }]
    });
    let _rel = server
        .mock(
            "GET",
            "/repos/giiyms/grog-build/releases/tags/grog-macos-aarch64",
        )
        .with_status(200)
        .with_body(release.to_string())
        .create_async()
        .await;
    let bin = server
        .mock("GET", "/dl/grog")
        .with_status(200)
        .with_body(payload.as_slice())
        .expect(0)
        .create_async()
        .await;
    let _sha = server
        .mock("GET", "/dl/sha")
        .with_status(200)
        .with_body(format!("{digest}  grog-1.0.9-macos-aarch64\n"))
        .create_async()
        .await;

    let opts = UpdateOptions {
        check_only: true,
        skip_platform_check: true,
        install_home: Some(grog_home.clone()),
        user_home: Some(tmp.path().to_path_buf()),
        api_base: Some(api),
        ..UpdateOptions::default()
    };
    let outcome = run_update(&opts).await.unwrap();
    match outcome {
        UpdateOutcome::Available { status } => {
            assert_eq!(status.name, "grog");
            assert_ne!(status.name, "grok");
            assert_eq!(status.latest_version, "1.0.9");
            assert!(status.update_available);
            assert_eq!(status.digest.as_deref(), Some(digest.as_str()));
        }
        other => panic!("{other:?}"),
    }
    bin.assert_async().await;
    assert!(!grog_home.exists());
}

#[tokio::test]
async fn checksum_mismatch_aborts_without_installing() {
    let tmp = tempfile::tempdir().unwrap();
    let grog_home = tmp.path().join(".grog");
    let mut server = mockito::Server::new_async().await;
    let api = server.url();
    let release = serde_json::json!({
        "tag_name": "grog-macos-aarch64",
        "name": "grog 1.0.9",
        "assets": [{
            "name": "grog-1.0.9-macos-aarch64",
            "browser_download_url": format!("{api}/dl/grog"),
            "size": 1
        }, {
            "name": "grog-1.0.9-macos-aarch64.sha256",
            "browser_download_url": format!("{api}/dl/sha"),
            "size": 80
        }]
    });
    let _rel = server
        .mock(
            "GET",
            "/repos/giiyms/grog-build/releases/tags/grog-macos-aarch64",
        )
        .with_status(200)
        .with_body(release.to_string())
        .create_async()
        .await;
    let _bin = server
        .mock("GET", "/dl/grog")
        .with_status(200)
        .with_body("tampered")
        .create_async()
        .await;
    let _sha = server
        .mock("GET", "/dl/sha")
        .with_status(200)
        .with_body(format!("{}  grog-1.0.9-macos-aarch64\n", sha(b"expected")))
        .create_async()
        .await;

    let err = run_update(&UpdateOptions {
        skip_platform_check: true,
        install_home: Some(grog_home.clone()),
        user_home: Some(tmp.path().to_path_buf()),
        api_base: Some(api),
        ..UpdateOptions::default()
    })
    .await
    .unwrap_err();
    assert!(
        matches!(err, grog_update::UpdateError::ChecksumMismatch { .. }),
        "{err}"
    );
    assert!(
        !grog_home
            .join("downloads/grog-1.0.9-macos-aarch64")
            .exists()
    );
}

#[tokio::test]
async fn refuses_install_home_under_grok() {
    let tmp = tempfile::tempdir().unwrap();
    let grok = tmp.path().join(".grok");
    std::fs::create_dir_all(&grok).unwrap();
    let err = run_update(&UpdateOptions {
        skip_platform_check: true,
        install_home: Some(grok),
        user_home: Some(tmp.path().to_path_buf()),
        api_base: Some("http://127.0.0.1:1".into()),
        ..UpdateOptions::default()
    })
    .await
    .unwrap_err();
    assert!(matches!(err, grog_update::UpdateError::GrokHomeForbidden));
}

#[test]
fn parse_sha256_text_is_public() {
    assert!(parse_sha256_text("dead").is_none());
}
