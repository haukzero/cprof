use crate::elevate::arguments::ContextReference;

use super::{ContextFile, MAX_BYTES, checksum, read_context};

#[test]
fn transports_answers_and_cleans_up_after_success_or_cancellation() {
    let home = tempfile::tempdir().unwrap();
    let args = vec![
        "unpack".into(),
        "--path".into(),
        "package with spaces.pkg".into(),
    ];
    let answers = serde_json::json!({"answers": [true, false]});
    let context = ContextFile::new(home.path(), &args, &answers).unwrap();
    let reference = context.reference();
    assert_eq!(
        read_context(&reference, home.path(), &args).unwrap(),
        answers
    );
    assert!(read_context(&reference, home.path(), &["other".into()]).is_err());
    assert!(read_context(&reference, &home.path().join("other"), &args).is_err());
    let mut corrupted = context.reference();
    corrupted.digest = "0".repeat(64);
    assert!(read_context(&corrupted, home.path(), &args).is_err());
    assert!(std::fs::write(&reference.path, b"changed").is_err());
    assert!(std::fs::remove_file(&reference.path).is_err());
    drop(context);
    assert!(!reference.path.exists());
}

#[test]
fn rejects_invalid_and_oversized_envelopes() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("context.json");
    for bytes in [b"{}".to_vec(), vec![b' '; MAX_BYTES as usize + 1]] {
        std::fs::write(&path, &bytes).unwrap();
        let reference = ContextReference {
            path: path.clone(),
            digest: checksum(&bytes),
        };
        assert!(read_context(&reference, home.path(), &[]).is_err());
    }
}
