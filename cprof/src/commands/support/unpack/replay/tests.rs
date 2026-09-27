use super::{DecisionKey, Replay, ReplaySession};

fn key(target: &str) -> DecisionKey {
    DecisionKey::OverwriteProfile {
        target: target.into(),
        profile: "shared".into(),
    }
}

fn replay(parent: &ReplaySession) -> ReplaySession {
    ReplaySession::new(Some(
        serde_json::from_value(parent.context([1; 32]).unwrap()).unwrap(),
    ))
    .unwrap()
}

#[test]
fn preserves_yes_no_and_operation_identity_without_prompting() {
    let mut parent = ReplaySession::default();
    let removal = DecisionKey::RemoveProfile {
        target: "codex".into(),
        profile: "shared".into(),
    };
    parent.decide(key("codex"), || Ok(true)).unwrap();
    parent.decide(key("claude"), || Ok(false)).unwrap();
    parent.decide(removal.clone(), || Ok(false)).unwrap();
    let mut child = replay(&parent);
    assert!(
        !child
            .decide(key("claude"), || panic!("prompted again"))
            .unwrap()
    );
    assert!(!child.decide(removal, || panic!("prompted again")).unwrap());
    assert!(
        child
            .decide(key("codex"), || panic!("prompted again"))
            .unwrap()
    );
    child.finish([1; 32]).unwrap();
    assert!(
        child
            .decide(key("codex"), || panic!("reused answer"))
            .is_err()
    );
}

#[test]
fn rejects_new_missing_changed_and_duplicate_records() {
    let mut parent = ReplaySession::default();
    parent.decide(key("codex"), || Ok(false)).unwrap();
    let mut child = replay(&parent);
    assert!(child.finish([1; 32]).is_err());
    assert!(
        child
            .decide(key("claude"), || panic!("new prompt"))
            .is_err()
    );
    assert!(
        !child
            .decide(key("codex"), || panic!("prompted again"))
            .unwrap()
    );
    assert!(child.finish([2; 32]).is_err());
    child.finish([1; 32]).unwrap();
    assert!(
        ReplaySession::new(Some(Replay {
            fingerprint: [1; 32],
            decisions: vec![(key("codex"), true), (key("codex"), false)],
        }))
        .is_err()
    );
}
