use std::fs;

use crate::support::{AUTH, CONFIG, TestHome, can_symlink, unmanaged_codex};

#[test]
fn profile_counts_include_incomplete_profiles() {
    let home = TestHome::new();
    home.claude_profile("complete");
    fs::create_dir_all(home.path.join(".cprof/profiles/claude/incomplete")).unwrap();

    assert_eq!(home.succeeds(&["claude", "num"]), "2 (1 incomplete)\n");

    let targets = home.succeeds(&["targets"]);
    let claude = targets
        .lines()
        .find(|line| line.starts_with("claude"))
        .expect("targets output should contain claude");
    assert!(claude.contains("2 (1 incomplete)"), "{targets}");
    assert!(targets.contains("profile count"), "{targets}");

    let json: serde_json::Value =
        serde_json::from_str(&home.succeeds(&["targets", "--json"])).unwrap();
    let claude = json
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["name"] == "claude")
        .expect("JSON output should contain claude");
    assert_eq!(claude["type"], "built-in");
    assert_eq!(claude["profiles"]["total"], 2);
    assert_eq!(claude["profiles"]["incomplete"], 1);
    assert!(claude["active"].is_null());
    assert_eq!(
        std::path::Path::new(claude["store_dir"].as_str().unwrap()),
        home.path.join(".cprof/profiles/claude")
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn non_interactive_remove_conflict_fails_before_deleting_any_profiles() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    home.claude_profile("active");
    home.claude_profile("inactive");

    home.succeeds(&["claude", "switch", "active"]);

    home.fails(
        &["claude", "remove", "inactive", "active"],
        "Interactive input required",
    );
    assert!(home.path.join(".cprof/profiles/claude/active").is_dir());
    assert!(home.path.join(".cprof/profiles/claude/inactive").is_dir());
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn forced_remove_deletes_an_active_profile_and_its_link() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    home.claude_profile("active");

    home.succeeds(&["claude", "switch", "active"]);

    home.succeeds(&["claude", "remove", "active", "--force"]);

    assert!(!home.path.join(".cprof/profiles/claude/active").exists());
    assert!(!home.path.join(".claude/settings.json").exists());
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn rename_updates_an_active_profile_and_its_links() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    home.claude_profile("old");

    home.succeeds(&["claude", "switch", "old"]);

    home.succeeds(&["claude", "rename", "old", "new"]);

    assert!(!home.path.join(".cprof/profiles/claude/old").exists());
    assert!(home.path.join(".cprof/profiles/claude/new").is_dir());
    let link = home.path.join(".claude/settings.json");
    assert_eq!(
        fs::read_link(link).unwrap(),
        home.path.join(".cprof/profiles/claude/new/settings.json")
    );
    assert!(home.succeeds(&["claude", "list"]).contains("new (active)"));
}

#[test]
fn rename_preserves_an_inactive_profile() {
    let home = TestHome::new();
    home.codex_profile("old");
    home.succeeds(&["codex", "rename", "old", "new"]);
    assert!(!home.path.join(".cprof/profiles/codex/old").exists());
    let profile = home.path.join(".cprof/profiles/codex/new");
    assert_eq!(
        fs::read_to_string(profile.join("config.toml")).unwrap(),
        CONFIG
    );
    assert_eq!(fs::read_to_string(profile.join("auth.json")).unwrap(), AUTH);
    assert!(!home.path.join(".codex").exists());
}

#[test]
fn remove_patterns_resolve_names_without_duplicates() {
    let home = TestHome::new();
    for name in ["work-first", "work-second", "personal"] {
        home.claude_profile(name);
    }
    let output = home.succeeds(&["claude", "remove", "work-*", "work-first"]);
    assert_eq!(output.matches("Removed profile").count(), 2);
    assert_eq!(home.succeeds(&["claude", "num"]), "1 (0 incomplete)\n");
    assert!(home.path.join(".cprof/profiles/claude/personal").is_dir());
    home.fails(&["claude", "remove", "missing_*"], "No profiles match");
    assert!(home.path.join(".cprof/profiles/claude/personal").is_dir());
}

#[test]
fn copy_from_preserves_resources_and_rejects_missing_or_incomplete_sources() {
    let home = unmanaged_codex();
    let source = home.codex_profile("source");
    home.succeeds(&["codex", "create", "copy", "-c", "source"]);
    for filename in ["config.toml", "auth.json"] {
        assert_eq!(
            fs::read(source.join(filename)).unwrap(),
            fs::read(home.path.join(".cprof/profiles/codex/copy").join(filename)).unwrap()
        );
    }
    home.fails(
        &["codex", "create", "missing-copy", "--copy-from", "missing"],
        "not found",
    );
    assert!(
        !home
            .path
            .join(".cprof/profiles/codex/missing-copy")
            .exists()
    );
    fs::remove_file(source.join("auth.json")).unwrap();
    home.fails(
        &["codex", "create", "incomplete-copy", "-c", "source"],
        "IO error",
    );
    assert!(
        !home
            .path
            .join(".cprof/profiles/codex/incomplete-copy")
            .exists()
    );
    assert_eq!(
        fs::read_to_string(source.join("config.toml")).unwrap(),
        CONFIG
    );
    assert!(!source.join("auth.json").exists());
}

#[test]
fn concurrent_profile_creation_has_a_single_winner() {
    use std::sync::Barrier;
    use std::thread;

    let home = unmanaged_codex();
    home.codex_profile("source");
    for iteration in 0..8 {
        let name = format!("concurrent-{iteration}");
        let barrier = Barrier::new(2);
        let results = thread::scope(|scope| {
            let workers = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        home.run(&["codex", "create", &name, "-c", "source"])
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            results
                .iter()
                .filter(|output| output.status.success())
                .count(),
            1,
            "{results:?}"
        );
        let failure = results
            .iter()
            .find(|output| !output.status.success())
            .unwrap();
        assert!(!failure.status.success(), "{results:?}");
        #[cfg(unix)]
        {
            let stderr = String::from_utf8_lossy(&failure.stderr);
            assert!(
                stderr.contains("already exists")
                    || stderr.contains("another cprof edit session is active"),
                "unexpected concurrent-create error: {stderr}"
            );
        }
        let profile = home.path.join(".cprof/profiles/codex").join(&name);
        assert_eq!(
            fs::read_to_string(profile.join("config.toml")).unwrap(),
            CONFIG
        );
        assert_eq!(fs::read_to_string(profile.join("auth.json")).unwrap(), AUTH);
        assert_eq!(fs::read_dir(profile).unwrap().count(), 2);
    }
    assert_eq!(
        fs::read_dir(home.path.join(".cprof/profiles/codex"))
            .unwrap()
            .count(),
        9
    );
}

#[test]
fn rename_conflicts_are_reported_before_changing_profiles() {
    let home = TestHome::new();
    home.claude_profile("old");
    home.claude_profile("existing");

    home.fails(&["claude", "rename", "old", "existing"], "already exists");
    assert!(home.path.join(".cprof/profiles/claude/old").is_dir());
    assert!(home.path.join(".cprof/profiles/claude/existing").is_dir());
}

#[test]
fn non_interactive_clean_requires_force_and_preserves_profiles() {
    let home = TestHome::new();
    home.claude_profile("active");

    home.fails(&["claude", "clean"], "Interactive input required");
    assert!(home.path.join(".cprof/profiles/claude/active").is_dir());
}
