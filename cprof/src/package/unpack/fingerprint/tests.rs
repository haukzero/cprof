use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::Fingerprint;

fn digest(path: &Path) -> [u8; 32] {
    let mut fingerprint = Fingerprint(Sha256::new());
    fingerprint.path(path).unwrap();
    fingerprint.finish()
}

#[test]
fn tracks_contents_and_missing_paths_without_timestamps() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("profile");
    let missing = digest(&path);
    fs::create_dir(&path).unwrap();
    let empty = digest(&path);
    assert_ne!(missing, empty);
    let resource = path.join("config.toml");
    fs::write(&resource, b"original").unwrap();
    let original = digest(&path);
    fs::write(&resource, b"changed").unwrap();
    assert_ne!(original, digest(&path));
    fs::remove_file(&resource).unwrap();
    assert_eq!(empty, digest(&path));
    fs::write(&resource, b"original").unwrap();
    assert_eq!(original, digest(&path));
    fs::write(path.join("extra"), b"original").unwrap();
    assert_ne!(original, digest(&path));
}
