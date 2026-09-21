use std::fs;

use crate::support::{INVALID_CONFIGS, TestHome};

#[test]
fn non_interactive_unpack_conflict_fails_before_commit() {
    let source = TestHome::new();
    source.claude_profile("existing");
    source.claude_profile("incoming");
    let package = source.path.join("profiles.pkg");
    source.succeeds(&["claude", "pack", "--save", package.to_str().unwrap()]);

    let destination = TestHome::new();
    destination.claude_profile("existing");
    destination.fails(
        &["claude", "unpack", "--path", package.to_str().unwrap()],
        "Interactive input required",
    );
    assert!(
        destination
            .path
            .join(".cprof/profiles/claude/existing")
            .is_dir()
    );
    assert!(
        !destination
            .path
            .join(".cprof/profiles/claude/incoming")
            .exists()
    );
}

#[test]
fn root_pack_can_select_multiple_targets() {
    let source = TestHome::new();
    source.set_config("[demo]\n");
    source.claude_profile("claude-profile");
    source.codex_profile("codex-profile");
    fs::create_dir_all(source.path.join(".cprof/profiles/demo/demo-profile")).unwrap();

    let output = source.succeeds(&[
        "pack", "--select", "claude", "--select", "demo", "--select", "claude",
    ]);
    assert!(output.contains("from 2 target(s)"), "{output}");

    let destination = TestHome::new();
    destination.succeeds(&[
        "unpack",
        "--path",
        source.path.join("cprof.pkg").to_str().unwrap(),
        "--force",
    ]);
    assert!(
        destination
            .path
            .join(".cprof/profiles/claude/claude-profile")
            .is_dir()
    );
    assert!(
        destination
            .path
            .join(".cprof/profiles/demo/demo-profile")
            .is_dir()
    );
    assert!(
        !destination
            .path
            .join(".cprof/profiles/codex/codex-profile")
            .exists()
    );
    assert!(
        fs::read_to_string(destination.config_path())
            .unwrap()
            .contains("[demo]")
    );
}

#[test]
fn root_pack_rejects_an_unknown_selected_target_before_writing() {
    let home = TestHome::new();
    home.claude_profile("test");

    home.fails(&["pack", "--select", "missing"], "Unknown target 'missing'");
    assert!(!home.path.join("cprof.pkg").exists());
}

#[test]
fn root_unpack_validates_local_config_before_merging() {
    let source = TestHome::new();
    let package = source.pack_external_target("[demo]\n", "demo");
    let destination = TestHome::new();

    for &(config, expected_error) in INVALID_CONFIGS {
        destination.set_config(config);
        destination.fails(
            &["unpack", "--path", package.to_str().unwrap(), "-f"],
            expected_error,
        );
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

    destination.fails(
        &["unpack", "--path", package.to_str().unwrap(), "-f"],
        "conflicts with an existing target",
    );
    assert_eq!(
        fs::read_to_string(destination.config_path()).unwrap(),
        config
    );
    assert!(!destination.path.join(".cprof/profiles").exists());
}

#[test]
fn root_unpack_staging_failure_leaves_config_and_profiles_unchanged() {
    let source = TestHome::new();
    source.set_config(
        "[demo]\nid = 'demo-id'\n[[demo.resources]]\nfilename = 'settings.conf'\nactive_path = '.demo/settings.conf'\n",
    );
    let codex = source.path.join(".cprof/profiles/codex/test");
    fs::create_dir_all(&codex).unwrap();
    fs::write(codex.join("config.toml"), "# valid\n").unwrap();
    fs::write(codex.join("auth.json"), "{}\n").unwrap();
    let external = source.path.join(".cprof/profiles/demo-id/test");
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("settings.conf"), "incoming").unwrap();
    source.succeeds(&["pack"]);

    let destination = TestHome::new();
    let profiles = destination.path.join(".cprof/profiles");
    fs::create_dir_all(&profiles).unwrap();
    fs::write(profiles.join("demo-id"), "blocks the target directory").unwrap();
    let output = destination.run(&[
        "unpack",
        "--path",
        source.path.join("cprof.pkg").to_str().unwrap(),
        "--force",
    ]);
    assert!(!output.status.success(), "{output:?}");
    #[cfg(unix)]
    {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Unsafe path") || stderr.contains("IO error"),
            "{stderr}"
        );
    }
    assert!(!destination.config_path().exists());
    assert!(!profiles.join("codex/test").exists());
    assert_eq!(
        fs::read_to_string(profiles.join("demo-id")).unwrap(),
        "blocks the target directory"
    );
}

#[test]
fn package_roundtrip_preserves_all_target_resources_and_overwrites_existing_profiles() {
    let source = TestHome::new();
    source.set_config(
        "[[demo.resources]]\nfilename = 'settings.conf'\nactive_path = '.demo/settings.conf'\n",
    );
    for name in ["first", "second"] {
        source.claude_profile(name);
        source.codex_profile(name);
        source.write_profile("demo", name, &[("settings.conf", name)]);
    }
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");
    let destination = TestHome::new();
    for force in [false, true] {
        let mut args = vec!["unpack", "--path", package.to_str().unwrap()];
        if force {
            destination.write_profile("codex", "first", &[("config.toml", "model = 'local'\n")]);
            args.push("--force");
        }
        destination.succeeds(&args);
        for target in ["claude", "codex", "demo"] {
            for name in ["first", "second"] {
                let relative = format!(".cprof/profiles/{target}/{name}");
                for entry in fs::read_dir(source.path.join(&relative)).unwrap() {
                    let entry = entry.unwrap();
                    assert_eq!(
                        fs::read(entry.path()).unwrap(),
                        fs::read(destination.path.join(&relative).join(entry.file_name())).unwrap()
                    );
                }
            }
        }
    }
}
