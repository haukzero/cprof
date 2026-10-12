use std::fs;

use crate::support::{TestHome, assert_failure, symlink};

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

#[cprof_macros::windows_elevation]
#[test]
fn removal_and_clean_failures_preserve_all_profiles_and_links() {
    for args in [
        &["demo", "remove", "work", "--force"][..],
        &["remove", "claude", "demo", "--force"],
        &["clean", "--extra-toml", "--force"],
    ] {
        let home = TestHome::new();
        let config = "[demo]\n\
            [[demo.resources]]\nfilename = 'first'\nactive_path = '.demo/first'\n\
            [[demo.resources]]\nfilename = 'second'\nactive_path = '.demo/second'\n\
            [[demo.resources]]\nfilename = 'third'\nactive_path = '.demo/third'\n";
        home.set_config(config);
        let claude = home.claude_profile("keep");
        home.succeeds(&["claude", "switch", "keep"]);
        let profile = home.write_profile("demo", "work", &[("first", "original")]);
        let external = home.path.join("external");
        fs::write(&external, "external").unwrap();
        symlink(&external, profile.join("third")).unwrap();
        fs::create_dir(home.path.join(".demo")).unwrap();
        let first = home.path.join(".demo/first");
        let third = home.path.join(".demo/third");
        symlink(profile.join("first"), &first).unwrap();
        fs::write(home.path.join(".demo/second"), "unmanaged").unwrap();
        symlink(profile.join("third"), &third).unwrap();
        // Status stops at the unmanaged second resource. Preparing removal of
        // the third then fails because stored resources may not be symlinks.
        let output = home.run(args);
        assert_failure(&output, "Unsafe path");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(!stdout.contains("Removed profile"), "{stdout}");
        assert!(!stdout.contains("Cleaned "), "{stdout}");
        assert!(claude.is_dir());
        assert_eq!(home.succeeds(&["claude", "which"]), "keep\n");
        assert_eq!(fs::read_link(&first).unwrap(), profile.join("first"));
        assert_eq!(fs::read_to_string(&first).unwrap(), "original");
        assert_eq!(fs::read_link(&third).unwrap(), profile.join("third"));
        assert_eq!(fs::read_to_string(&external).unwrap(), "external");
        assert_eq!(
            fs::read_to_string(home.path.join(".demo/second")).unwrap(),
            "unmanaged"
        );
        assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
        assert_eq!(
            fs::read_dir(home.path.join(".cprof/profiles/claude"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            fs::read_dir(home.path.join(".cprof/profiles/demo"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn root_clean_commits_profiles_links_and_extra_config_together() {
    let home = crate::support::unmanaged_codex();
    home.set_config("[demo]\n");
    home.claude_profile("active");
    home.codex_profile("inactive");
    home.write_profile("demo", "empty", &[]);
    home.succeeds(&["claude", "switch", "active"]);

    let output = home.succeeds(&["clean", "--extra-toml", "--force"]);
    assert_eq!(
        output.matches("Cleaned 1 profile(s)").count(),
        3,
        "{output}"
    );
    for target in ["claude", "codex", "demo"] {
        assert!(!home.path.join(".cprof/profiles").join(target).exists());
    }
    assert!(!home.config_path().exists());
    assert!(fs::symlink_metadata(home.path.join(".claude/settings.json")).is_err());
    crate::support::assert_originals(&home);
}
