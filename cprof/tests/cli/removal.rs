use std::fs;

use crate::support::TestHome;

#[test]
fn root_remove_combines_targets_and_profile_patterns_without_duplicates() {
    let home = TestHome::new();
    let config = "[example]\nid = 'custom-target'\n";
    home.set_config(config);
    for target in ["claude", "codex", "custom-target"] {
        for name in ["personal", "work-first", "work-second"] {
            home.write_profile(target, name, &[]);
        }
    }
    let output = home.succeeds(&[
        "remove",
        "claude/work-first",
        "claude",
        "claude/work-second",
        "codex/work-*",
        "codex/work-first",
        "custom-target/personal",
    ]);
    assert_eq!(output.matches("Removed profile").count(), 6);
    for target in ["claude", "codex", "custom-target"] {
        for name in ["personal", "work-first", "work-second"] {
            let removed = match target {
                "claude" => true,
                "codex" => name.starts_with("work-"),
                _ => name == "personal",
            };
            let path = format!("{target}/{name}");
            assert_eq!(
                output.contains(&format!("Removed profile '{path}'")),
                removed
            );
            assert_eq!(
                home.path.join(".cprof/profiles").join(path).exists(),
                !removed
            );
        }
    }
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
}

#[test]
fn remove_supports_external_targets_and_unicode_profile_patterns() {
    for root in [true, false] {
        let home = TestHome::new();
        home.set_config("[example]\nid = 'custom-target'\n");
        for name in ["工作 一", "工作 二", "personal"] {
            home.write_profile("custom-target", name, &[]);
        }
        let output = if root {
            home.succeeds(&["remove", "custom-target/工作 ?", "custom-target/工作 一"])
        } else {
            home.succeeds(&["custom-target", "remove", "工作 ?", "工作 一"])
        };
        assert_eq!(output.matches("Removed profile").count(), 2);
        assert!(
            !home
                .path
                .join(".cprof/profiles/custom-target/工作 一")
                .exists()
        );
        assert!(
            !home
                .path
                .join(".cprof/profiles/custom-target/工作 二")
                .exists()
        );
        assert!(
            home.path
                .join(".cprof/profiles/custom-target/personal")
                .is_dir()
        );
    }
}

#[test]
fn invalid_removal_selections_fail_before_deleting_any_profiles() {
    let home = TestHome::new();
    home.claude_profile("keep");
    home.codex_profile("keep");
    for (selector, error) in [
        ("missing/profile", "Unknown target 'missing'"),
        ("codex/missing", "Profile 'missing' not found"),
        ("codex/missing-*", "No profiles matched"),
        ("codex/", "Invalid profile name"),
        ("codex/../keep", "Invalid profile name"),
        ("claude/missing", "Profile 'missing' not found"),
    ] {
        home.fails(&["remove", "claude", selector, "--force"], error);
        assert!(home.path.join(".cprof/profiles/claude/keep").is_dir());
        assert!(home.path.join(".cprof/profiles/codex/keep").is_dir());
    }
    for (selector, error) in [
        ("missing", "Profile 'missing' not found"),
        ("../keep", "Invalid profile name"),
        ("missing-*", "No profiles matched"),
    ] {
        home.fails(&["claude", "remove", "keep", selector, "--force"], error);
        assert!(home.path.join(".cprof/profiles/claude/keep").is_dir());
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn root_remove_confirms_all_active_profiles_before_deleting_from_any_target() {
    let home = TestHome::new();
    home.claude_profile("inactive");
    home.codex_profile("active");
    home.succeeds(&["codex", "switch", "active"]);

    home.fails(&["remove", "claude", "codex"], "Interactive input required");
    assert!(home.path.join(".cprof/profiles/claude/inactive").is_dir());
    assert!(home.path.join(".cprof/profiles/codex/active").is_dir());
    for filename in ["config.toml", "auth.json"] {
        assert!(home.path.join(".codex").join(filename).is_symlink());
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn forced_root_remove_clears_selected_active_links_and_preserves_other_profiles() {
    let home = TestHome::new();
    home.claude_profile("active");
    home.claude_profile("keep");
    home.codex_profile("active");
    home.codex_profile("other");
    home.succeeds(&["claude", "switch", "active"]);
    home.succeeds(&["codex", "switch", "active"]);

    let output = home.succeeds(&["remove", "claude/active", "codex/other", "-f"]);
    assert!(output.contains("Removed profile 'claude/active' and cleared active links"));
    assert!(output.contains("Removed profile 'codex/other'"));
    assert!(!home.path.join(".cprof/profiles/claude/active").exists());
    assert!(!home.path.join(".cprof/profiles/codex/other").exists());
    assert!(home.path.join(".cprof/profiles/claude/keep").is_dir());
    assert!(fs::symlink_metadata(home.path.join(".claude/settings.json")).is_err());
    assert_eq!(home.succeeds(&["codex", "which"]).trim(), "active");

    home.succeeds(&["remove", "codex", "--force"]);
    assert!(!home.path.join(".cprof/profiles/codex/active").exists());
    for filename in ["config.toml", "auth.json"] {
        assert!(fs::symlink_metadata(home.path.join(".codex").join(filename)).is_err());
    }
}

#[test]
fn empty_remove_trees_report_no_profiles_and_force_still_requires_a_selection() {
    let home = TestHome::new();
    for args in [&["remove"][..], &["claude", "remove"]] {
        home.fails(args, "no profiles exist");
    }
    home.claude_profile("keep");
    for args in [
        &["remove", "--force", "--ascii"][..],
        &["claude", "remove", "--force", "--ascii"],
    ] {
        home.fails(args, "Interactive input required");
        assert!(home.path.join(".cprof/profiles/claude/keep").is_dir());
    }
    home.succeeds(&["remove", "codex"]);
    assert!(home.path.join(".cprof/profiles/claude/keep").is_dir());
}
