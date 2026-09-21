use std::fs;

use crate::support::{INVALID_CONFIGS, TestHome};

#[test]
fn static_cli_information_does_not_require_valid_config() {
    let home = TestHome::new();
    home.set_config("[broken");
    for args in [
        &["--version"][..],
        &["edit-extra", "--help"],
        &["help", "edit-extra"],
        &["pack", "--help"],
    ] {
        assert!(!home.succeeds(args).is_empty());
    }
}

#[test]
fn non_interactive_prompts_report_a_consistent_error() {
    let home = TestHome::new();
    home.claude_profile("test");

    for args in [
        &["pack", "--select"][..],
        &["claude", "switch"],
        &["claude", "where"],
        &["claude", "create"],
    ] {
        home.fails(args, "Interactive input required");
    }
    assert!(!home.path.join("cprof.pkg").exists());
}

#[test]
fn external_targets_are_resolved_and_listed_in_root_help() {
    let home = TestHome::new();
    home.set_config("[example]\nid = 'custom-target'\n");

    let output = home.succeeds(&["custom-target", "dir"]);
    assert_eq!(
        std::path::Path::new(output.trim()),
        home.path.join(".cprof/profiles/custom-target")
    );
    for args in [&["--help"][..], &["-h"], &["help"], &[]] {
        let help = home.succeeds(args);
        assert!(help.contains("custom-target"), "{help}");
        assert!(help.contains("edit-extra"), "{help}");
    }
    assert!(
        home.succeeds(&["custom-target", "--help"])
            .contains("switch")
    );

    for unknown in ["example", "missing-target"] {
        home.fails(&[unknown, "dir"], &format!("Unknown target '{unknown}'"));
    }
}

#[test]
fn target_consumers_still_reject_invalid_configs() {
    let home = TestHome::new();
    for &(config, expected_error) in INVALID_CONFIGS {
        home.set_config(config);
        for args in [
            &["targets"][..],
            &["claude", "dir"],
            &["demo", "dir"],
            &["pack"],
            &["clean", "-f", "--extra-toml"],
            &["--help"],
        ] {
            home.fails(args, expected_error);
            assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
            assert!(!home.path.join("cprof.pkg").exists());
        }
    }
}
