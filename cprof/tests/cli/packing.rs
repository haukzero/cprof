use std::fs;

use crate::support::TestHome;

fn unpack(source: &TestHome) -> TestHome {
    let destination = TestHome::new();
    destination.succeeds(&[
        "unpack",
        "--path",
        source.path.join("cprof.pkg").to_str().unwrap(),
        "--force",
    ]);
    destination
}

fn profile_names(home: &TestHome, target: &str) -> Vec<String> {
    let dir = home.path.join(".cprof/profiles").join(target);
    if !dir.exists() {
        return vec![];
    }
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn root_pack_combines_whole_targets_and_individual_profiles() {
    let source = TestHome::new();
    source.set_config("[example]\nid = 'custom-target'\n");
    for name in ["personal", "work"] {
        source.claude_profile(name);
        source.codex_profile(name);
        source.write_profile("custom-target", name, &[]);
    }
    let output = source.succeeds(&[
        "pack",
        "--select",
        "claude",
        "--select",
        "codex/work",
        "--select",
        "custom-target/personal",
        "--select",
        "codex/work",
    ]);
    assert!(
        output.contains("Packed 4 profile(s) from 3 target(s)"),
        "{output}"
    );
    let destination = unpack(&source);
    assert_eq!(profile_names(&destination, "claude"), ["personal", "work"]);
    assert_eq!(profile_names(&destination, "codex"), ["work"]);
    assert_eq!(profile_names(&destination, "custom-target"), ["personal"]);
    assert!(
        fs::read_to_string(destination.config_path())
            .unwrap()
            .contains("custom-target")
    );
}

#[test]
fn overlapping_target_and_profile_selections_are_a_union_in_either_order() {
    let source = TestHome::new();
    source.claude_profile("personal");
    source.claude_profile("work");
    for selectors in [["claude", "claude/work"], ["claude/work", "claude"]] {
        let output = source.succeeds(&["pack", "--select", selectors[0], "--select", selectors[1]]);
        assert!(
            output.contains("Packed 2 profile(s) from 1 target(s)"),
            "{output}"
        );
        assert_eq!(
            profile_names(&unpack(&source), "claude"),
            ["personal", "work"]
        );
    }
}

#[test]
fn target_pack_selects_profiles_for_builtin_and_external_targets() {
    let source = TestHome::new();
    source.set_config("[example]\nid = 'custom-target'\n");
    for target in ["claude", "custom-target"] {
        for name in ["claude", "工作 配置", "excluded"] {
            source.write_profile(target, name, &[("settings.json", "{}\n")]);
        }
        let output = source.succeeds(&[
            target,
            "pack",
            "--select",
            "claude",
            "--select",
            "工作 配置",
            "--select",
            "claude",
        ]);
        assert!(
            output.contains("Packed 2 profile(s) from 1 target(s)"),
            "{output}"
        );
        let destination = unpack(&source);
        assert_eq!(profile_names(&destination, target), ["claude", "工作 配置"]);
        assert!(profile_names(&destination, "codex").is_empty());
    }
}

#[test]
fn excluded_invalid_profiles_do_not_prevent_packing_selected_profiles() {
    let source = TestHome::new();
    source.claude_profile("valid");
    source.write_profile("claude", "invalid", &[("settings.json", "invalid json")]);
    source.write_profile("claude", "incomplete", &[]);
    source.write_profile("codex", "unselected-target", &[]);
    for args in [
        &["pack", "--select", "claude/valid"][..],
        &["claude", "pack", "--select", "valid"],
    ] {
        source.succeeds(args);
        let destination = unpack(&source);
        assert_eq!(profile_names(&destination, "claude"), ["valid"]);
        assert!(profile_names(&destination, "codex").is_empty());
    }
}

#[test]
fn invalid_selections_and_invalid_selected_profiles_preserve_existing_output() {
    let source = TestHome::new();
    source.claude_profile("valid");
    source.write_profile("claude", "incomplete", &[]);
    source.write_profile("claude", "invalid", &[("settings.json", "invalid json")]);
    let output = source.path.join("existing package.pkg");
    fs::write(&output, b"keep the existing output").unwrap();
    for (args, error) in [
        (
            &["pack", "--select", "missing/profile"][..],
            "Unknown target 'missing'",
        ),
        (
            &["pack", "--select", "claude/missing"],
            "Profile 'missing' not found",
        ),
        (&["pack", "--select", "claude/"], "Invalid profile name"),
        (
            &["pack", "--select", "claude/../valid"],
            "Invalid profile name",
        ),
        (
            &["pack", "--select", "claude", "--select", "claude/missing"],
            "Profile 'missing' not found",
        ),
        (
            &["claude", "pack", "--select", "missing"],
            "Profile 'missing' not found",
        ),
        (
            &["claude", "pack", "--select", "../valid"],
            "Invalid profile name",
        ),
        (
            &[
                "claude",
                "pack",
                "--select",
                "valid",
                "--select",
                "incomplete",
            ],
            "is incomplete",
        ),
        (&["claude", "pack", "--select", "invalid"], "Invalid JSON"),
    ] {
        let mut args = args.to_vec();
        args.extend(["--save", output.to_str().unwrap()]);
        source.fails(&args, error);
        assert_eq!(fs::read(&output).unwrap(), b"keep the existing output");
    }
}

#[test]
fn empty_targets_cannot_overwrite_a_package() {
    let source = TestHome::new();
    let output = source.path.join("cprof.pkg");
    fs::write(&output, b"existing package").unwrap();
    for args in [
        &["pack"][..],
        &["pack", "--select", "claude"],
        &["claude", "pack"],
        &["pack", "--select"],
        &["claude", "pack", "--select"],
    ] {
        source.fails(args, "No profiles to pack");
        assert_eq!(fs::read(&output).unwrap(), b"existing package");
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn selected_packages_record_an_active_profile_only_when_it_is_included() {
    let source = TestHome::new();
    source.claude_profile("active");
    source.claude_profile("other");
    source.succeeds(&["claude", "switch", "active"]);
    for args in [
        &["pack", "--select", "claude/other"][..],
        &["claude", "pack", "--select", "other"],
        &["pack", "--select", "claude/active"],
        &["claude", "pack", "--select", "active"],
    ] {
        source.succeeds(args);
        let destination = TestHome::new();
        destination.succeeds(&[
            "unpack",
            "--mirror",
            "--force",
            "--path",
            source.path.join("cprof.pkg").to_str().unwrap(),
        ]);
        let included = args.last() == Some(&"active") || args.last() == Some(&"claude/active");
        if included {
            assert_eq!(destination.succeeds(&["claude", "which"]).trim(), "active");
        } else {
            assert_eq!(profile_names(&destination, "claude"), ["other"]);
            assert!(!destination.path.join(".claude/settings.json").exists());
        }
    }
}
