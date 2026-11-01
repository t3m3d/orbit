use super::*;

#[test]
fn destination_preflight_rejects_empty_and_relative_paths() {
    assert_eq!(
        storage_preflight("", 1).expect_err("empty path must fail"),
        "Choose a destination folder."
    );
    assert_eq!(
        storage_preflight("relative-folder", 1).expect_err("relative destination must fail"),
        "The destination must be an absolute path."
    );
}

#[test]
fn destination_preflight_rejects_an_existing_file() {
    let path = std::env::temp_dir().join(format!("orbit-destination-file-{}", std::process::id()));
    std::fs::write(&path, b"not a directory").expect("create destination test file");
    let error =
        storage_preflight(&path.to_string_lossy(), 1).expect_err("file destination must fail");
    assert_eq!(error, "The destination points to a file, not a folder.");
    std::fs::remove_file(path).expect("remove destination test file");
}

#[test]
fn only_supported_transfer_sources_are_accepted() {
    assert!(is_supported_source("magnet:?xt=urn:btih:abc"));
    assert!(is_supported_source("https://example.test/legal.torrent"));
    assert!(is_supported_source(r"C:\Downloads\LEGAL.TORRENT"));
    assert!(!is_supported_source(r"C:\Downloads\payload.exe"));
    assert!(!is_supported_source("ftp://example.test/file.torrent"));
}

#[cfg(unix)]
#[test]
fn destination_preflight_checks_volume_space_for_new_folders() {
    let path = std::env::temp_dir()
        .join(format!("orbit-new-destination-{}", std::process::id()))
        .join("nested");
    let available = storage_preflight(&path.to_string_lossy(), 0)
        .expect("existing parent volume can be checked")
        .expect("Unix reports disk space");
    assert!(available > 0);
    assert!(
        storage_preflight(&path.to_string_lossy(), u64::MAX)
            .expect_err("oversized download must fail")
            .starts_with("Not enough free space:")
    );
    assert!(!path.exists(), "preflight must not create folders");
}
