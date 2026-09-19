// These process tests isolate dirs::home_dir() via HOME and use a shell editor.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
#[path = "cli/paths.rs"]
mod paths;
mod support;

use support::TestHome;

const INVALID_CONFIGS: &[(&str, &str)] = &[
    ("[broken", "Failed to parse"),
    ("[demo]\nid = 1\n", "Failed to parse"),
    ("[claude]\n", "conflicts with an existing target"),
    (
        "[edit-extra]\n",
        "conflicts with the command of the same name",
    ),
    ("[help]\n", "conflicts with the command of the same name"),
    (
        "[first]\nid = 'same'\n[second]\nid = 'same'\n",
        "conflicts with an existing target",
    ),
    (
        "[[demo.resources]]\nfilename = 'a'\nactive_path = '.demo/a'\n\
         [[demo.resources]]\nkey = 'a'\nfilename = 'b'\nactive_path = '.demo/b'\n",
        "declares duplicate resource",
    ),
];

#[test]
fn edit_extra_opens_invalid_configs_without_replacing_them() {
    let home = TestHome::new();
    let editor = home.path.join("test editor");
    fs::write(&editor, "#!/bin/sh\nprintf '%s\\n' \"$1\"\n").unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();

    for &(config, _) in INVALID_CONFIGS {
        home.set_config(config);
        let output = home.succeeds(&["edit-extra", "--editor", editor.to_str().unwrap()]);
        assert_eq!(output.trim(), home.config_path().to_str().unwrap());
        assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
    }
}

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
fn external_targets_are_resolved_and_listed_in_root_help() {
    let home = TestHome::new();
    home.set_config("[example]\nid = 'custom-target'\n");

    let output = home.succeeds(&["custom-target", "dir"]);
    assert_eq!(
        output.trim(),
        home.path
            .join(".cprof/profiles/custom-target")
            .to_str()
            .unwrap()
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
        let output = home.run(&[unknown, "dir"]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains(&format!("Unknown target '{unknown}'")),
            "{output:?}"
        );
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
            let stderr = home.fails(args);
            assert!(
                stderr.contains(expected_error),
                "{config}: {args:?}: {stderr}"
            );
            assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
            assert!(!home.path.join("cprof.pkg").exists());
        }
    }
}

#[test]
fn root_unpack_validates_local_config_before_merging() {
    let source = TestHome::new();
    let package = source.pack_external_target("[demo]\n", "demo");
    let destination = TestHome::new();

    for &(config, expected_error) in INVALID_CONFIGS {
        destination.set_config(config);
        let stderr = destination.fails(&["unpack", "--path", package.to_str().unwrap(), "-f"]);
        assert!(stderr.contains(expected_error), "{config}: {stderr}");
        assert_eq!(
            fs::read_to_string(destination.config_path()).unwrap(),
            config
        );
        assert!(!destination.path.join(".cprof/profiles").exists());
    }

    destination.set_config("");
    destination.succeeds(&["unpack", "--path", package.to_str().unwrap(), "-f"]);
    assert!(destination.path.join(".cprof/profiles/demo/test").is_dir());
    assert!(destination.succeeds(&["--help"]).contains("demo"));
}

#[test]
fn root_unpack_rejects_duplicate_ids_created_by_merging() {
    let source = TestHome::new();
    let package = source.pack_external_target("[incoming]\nid = 'shared'\n", "shared");
    let destination = TestHome::new();
    let config = "[incoming]\nid = 'old'\n[other]\nid = 'shared'\n";
    destination.set_config(config);

    let stderr = destination.fails(&["unpack", "--path", package.to_str().unwrap(), "-f"]);
    assert!(
        stderr.contains("conflicts with an existing target"),
        "{stderr}"
    );
    assert_eq!(
        fs::read_to_string(destination.config_path()).unwrap(),
        config
    );
    assert!(!destination.path.join(".cprof/profiles").exists());
}
