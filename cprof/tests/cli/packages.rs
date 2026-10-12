use std::fs;

use crate::support::{INVALID_CONFIGS, TestHome, symlink};

#[test]
fn non_interactive_unpack_ignores_identical_profile_and_restores_changes() {
    let source = TestHome::new();
    source.claude_profile("existing");
    source.claude_profile("incoming");
    let package = source.path.join("profiles.pkg");
    source.succeeds(&["claude", "pack", "--save", package.to_str().unwrap()]);

    let destination = TestHome::new();
    destination.claude_profile("existing");
    let output = destination.succeeds(&["claude", "unpack", "--path", package.to_str().unwrap()]);
    assert!(!output.contains("conflict"), "{output}");
    assert!(!output.contains("Overwrite"), "{output}");
    assert!(
        destination
            .path
            .join(".cprof/profiles/claude/existing")
            .is_dir()
    );
    assert!(
        destination
            .path
            .join(".cprof/profiles/claude/incoming")
            .is_dir()
    );
}

#[test]
#[cfg(unix)]
fn unpack_identical_profile_is_silent_and_unchanged() {
    let source = TestHome::new();
    source.claude_profile("same");
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");

    let destination = TestHome::new();
    destination.claude_profile("same");
    let output = destination.succeeds(&["unpack", "--path", package.to_str().unwrap()]);

    assert!(!output.contains("conflict"), "{output}");
    assert!(!output.contains("Overwrite"), "{output}");
    assert!(!output.contains("Unpacking target"), "{output}");
    assert!(!output.contains("Unpacked target"), "{output}");
    assert!(
        output.contains("0 unpacked, 1 unchanged, 0 skipped across 1 target(s)"),
        "{output}"
    );
}

#[test]
#[cfg(unix)]
fn root_unpack_omits_targets_without_changes() {
    let source = TestHome::new();
    source.claude_profile("same");
    source.codex_profile("incoming");
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");

    let destination = TestHome::new();
    destination.claude_profile("same");
    let output = destination.succeeds(&["unpack", "--path", package.to_str().unwrap()]);

    assert!(!output.contains("Preparing target 'claude'"), "{output}");
    assert!(!output.contains("Unpacked target 'claude'"), "{output}");
    assert!(output.contains("Preparing target 'codex'..."), "{output}");
    assert!(output.contains("Unpacked target 'codex':"), "{output}");
    assert!(output.contains("  Unpacked profile 'incoming'"), "{output}");
}

#[test]
#[cfg(unix)]
fn root_unpack_groups_profiles_under_their_targets() {
    let source = TestHome::new();
    source.claude_profile("shared");
    source.codex_profile("shared");
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");

    let destination = TestHome::new();
    let output = destination.succeeds(&["unpack", "--path", package.to_str().unwrap(), "--force"]);

    assert!(
        output.contains(
            "Unpacked target 'claude':\n  Unpacked profile 'shared'\nUnpacked target 'codex':\n  Unpacked profile 'shared'"
        ),
        "{output}"
    );
    assert!(output.contains("Preparing target 'claude'..."), "{output}");
    assert!(output.contains("Preparing target 'codex'..."), "{output}");
}

#[test]
fn target_unpack_dry_run_reports_changes_without_writing() {
    let source = TestHome::new();
    source.write_profile(
        "claude",
        "existing",
        &[("settings.json", "{\"incoming\":true}\n")],
    );
    source.claude_profile("new");
    source.claude_profile("same");
    source.succeeds(&["claude", "pack"]);
    let package = source.path.join("cprof.pkg");

    let destination = TestHome::new();
    destination.write_profile(
        "claude",
        "existing",
        &[("settings.json", "{\"local\":true}\n")],
    );
    destination.claude_profile("same");
    let output = destination.succeeds(&[
        "claude",
        "unpack",
        "--path",
        package.to_str().unwrap(),
        "--dry-run",
    ]);

    assert!(
        output.contains("target 'claude':\n  M profile 'existing'"),
        "{output}"
    );
    assert!(output.contains("  + profile 'new'"), "{output}");
    assert!(!output.contains("profile 'same'"), "{output}");
    assert!(!output.contains(".cprof"), "{output}");
    assert!(!output.contains("Preparing target"), "{output}");
    assert_eq!(
        fs::read_to_string(
            destination
                .path
                .join(".cprof/profiles/claude/existing/settings.json")
        )
        .unwrap(),
        "{\"local\":true}\n"
    );
    assert!(!destination.path.join(".cprof/profiles/claude/new").exists());
}

#[cprof_macros::windows_elevation]
#[test]
fn mirror_unpack_replaces_profile_names_and_active_configuration() {
    let source = TestHome::new();
    source.claude_profile("new");
    source.succeeds(&["claude", "switch", "new"]);
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");

    let destination = TestHome::new();
    destination.claude_profile("old");
    let dry_run = destination.succeeds(&[
        "claude",
        "unpack",
        "--path",
        package.to_str().unwrap(),
        "--mirror",
        "--dry-run",
    ]);
    assert!(
        dry_run.contains("target 'claude':\n  + profile 'new'")
            && dry_run.contains("  - profile 'old'")
            && dry_run.contains("  + active profile 'new'"),
        "{dry_run}"
    );
    assert!(destination.path.join(".cprof/profiles/claude/old").is_dir());

    destination.succeeds(&[
        "claude",
        "unpack",
        "--path",
        package.to_str().unwrap(),
        "--mirror",
        "--force",
    ]);
    assert!(!destination.path.join(".cprof/profiles/claude/old").exists());
    assert!(destination.path.join(".cprof/profiles/claude/new").is_dir());
    let active = destination.path.join(".claude/settings.json");
    assert!(active.is_symlink());
    assert_eq!(
        fs::canonicalize(active).unwrap(),
        fs::canonicalize(
            destination
                .path
                .join(".cprof/profiles/claude/new/settings.json")
        )
        .unwrap()
    );
}

#[test]
fn root_mirror_removes_targets_absent_from_the_package() {
    let source = TestHome::new();
    source.claude_profile("kept");
    source.succeeds(&["pack"]);
    let package = source.path.join("cprof.pkg");

    let destination = TestHome::new();
    destination.codex_profile("stale");
    destination.succeeds(&[
        "unpack",
        "--path",
        package.to_str().unwrap(),
        "--mirror",
        "--force",
    ]);
    assert!(
        destination
            .path
            .join(".cprof/profiles/claude/kept")
            .is_dir()
    );
    assert!(
        !destination
            .path
            .join(".cprof/profiles/codex/stale")
            .exists()
    );
}

#[test]
fn mirror_previews_and_clears_unmanaged_configuration() {
    let source = TestHome::new();
    source.codex_profile("work");
    source.succeeds(&["codex", "pack"]);
    let package = source.path.join("cprof.pkg");
    let destination = crate::support::unmanaged_codex();
    destination.codex_profile("work");
    let args = [
        "codex",
        "unpack",
        "--path",
        package.to_str().unwrap(),
        "--mirror",
    ];

    let preview = destination.succeeds(&[&args[..], &["--dry-run"]].concat());
    assert!(preview.contains("  - active configuration"), "{preview}");
    crate::support::assert_originals(&destination);

    destination.succeeds(&[&args[..], &["--force"]].concat());
    for filename in ["config.toml", "auth.json"] {
        assert!(fs::symlink_metadata(destination.path.join(".codex").join(filename)).is_err());
    }
    assert!(
        destination
            .succeeds(&[&args[..], &["--dry-run"]].concat())
            .is_empty()
    );
}

#[cprof_macros::windows_elevation]
#[test]
fn mirror_previews_and_clears_managed_link_states() {
    let source = TestHome::new();
    source.codex_profile("first");
    source.codex_profile("second");
    source.succeeds(&["codex", "pack"]);
    let package = source.path.join("cprof.pkg");
    for state in ["active", "partial", "mixed", "dangling"] {
        let destination = TestHome::new();
        let first = destination.codex_profile("first");
        let second = destination.codex_profile("second");
        destination.succeeds(&["codex", "switch", "first"]);
        let auth = destination.path.join(".codex/auth.json");
        match state {
            "partial" => fs::remove_file(&auth).unwrap(),
            "mixed" => {
                fs::remove_file(&auth).unwrap();
                symlink(second.join("auth.json"), &auth).unwrap();
            }
            "dangling" => fs::remove_file(first.join("auth.json")).unwrap(),
            _ => {}
        }
        let before = fs::read_link(&auth).ok();
        let args = [
            "codex",
            "unpack",
            "--path",
            package.to_str().unwrap(),
            "--mirror",
        ];
        let preview = destination.succeeds(&[&args[..], &["--dry-run"]].concat());
        let expected = if state == "active" {
            "  - active profile 'first'"
        } else {
            "  - active configuration"
        };
        assert!(preview.contains(expected), "{state}: {preview}");
        assert_eq!(fs::read_link(&auth).ok(), before);
        assert_eq!(first.join("auth.json").exists(), state != "dangling");

        destination.succeeds(&[&args[..], &["--force"]].concat());
        assert!(first.is_dir());
        assert!(second.is_dir());
        for filename in ["config.toml", "auth.json"] {
            assert!(fs::symlink_metadata(destination.path.join(".codex").join(filename)).is_err());
        }
        assert!(
            destination
                .succeeds(&[&args[..], &["--dry-run"]].concat())
                .is_empty()
        );
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn mirror_repairs_a_profile_with_existing_dangling_links() {
    let source = TestHome::new();
    source.codex_profile("work");
    source.succeeds(&["codex", "switch", "work"]);
    source.succeeds(&["codex", "pack"]);
    let package = source.path.join("cprof.pkg");
    let destination = TestHome::new();
    let profile = destination.codex_profile("work");
    destination.succeeds(&["codex", "switch", "work"]);
    fs::remove_file(profile.join("auth.json")).unwrap();
    let auth = destination.path.join(".codex/auth.json");
    let link = fs::read_link(&auth).unwrap();
    let args = [
        "codex",
        "unpack",
        "--path",
        package.to_str().unwrap(),
        "--mirror",
    ];

    let preview = destination.succeeds(&[&args[..], &["--dry-run"]].concat());
    assert!(preview.contains("  M active profile 'work'"), "{preview}");
    assert!(!auth.exists());
    destination.succeeds(&[&args[..], &["--force"]].concat());
    assert_eq!(fs::read_link(&auth).unwrap(), link);
    assert_eq!(destination.succeeds(&["codex", "which"]), "work\n");
}

#[cprof_macros::windows_elevation]
#[test]
fn mirror_reconciles_optional_links_for_the_same_active_profile() {
    let config = "[demo]\n[[demo.resources]]\nfilename = 'settings'\nactive_path = '.demo/settings'\n\
        [[demo.resources]]\nfilename = 'optional'\nactive_path = '.demo/optional'\nrequired = false\n";
    for incoming_optional in [false, true] {
        let source = TestHome::new();
        source.set_config(config);
        let incoming = source.write_profile("demo", "work", &[("settings", "same")]);
        if incoming_optional {
            fs::write(incoming.join("optional"), "incoming").unwrap();
        }
        source.succeeds(&["demo", "switch", "work"]);
        source.succeeds(&["demo", "pack"]);
        let package = source.path.join("cprof.pkg");

        let destination = TestHome::new();
        destination.set_config(config);
        let local = destination.write_profile("demo", "work", &[("settings", "same")]);
        if !incoming_optional {
            fs::write(local.join("optional"), "local").unwrap();
        }
        destination.succeeds(&["demo", "switch", "work"]);
        let optional = destination.path.join(".demo/optional");
        let args = [
            "demo",
            "unpack",
            "--path",
            package.to_str().unwrap(),
            "--mirror",
        ];

        let preview = destination.succeeds(&[&args[..], &["--dry-run"]].concat());
        assert!(preview.contains("  M active profile 'work'"), "{preview}");
        assert_eq!(optional.is_symlink(), !incoming_optional);
        destination.succeeds(&[&args[..], &["--force"]].concat());
        assert_eq!(optional.is_symlink(), incoming_optional);
        if incoming_optional {
            assert_eq!(fs::read_to_string(&optional).unwrap(), "incoming");
        } else {
            assert!(fs::symlink_metadata(&optional).is_err());
        }
        assert_eq!(destination.succeeds(&["demo", "which"]), "work\n");
        assert!(
            destination
                .succeeds(&[&args[..], &["--dry-run"]].concat())
                .is_empty()
        );
    }
}

#[test]
fn root_unpack_dry_run_reports_external_config_without_writing() {
    let source = TestHome::new();
    let package = source.pack_external_target("[demo]\n", "demo");
    let destination = TestHome::new();

    let output =
        destination.succeeds(&["unpack", "--path", package.to_str().unwrap(), "--dry-run"]);

    assert!(
        output.contains("target 'demo':\n  + configuration"),
        "{output}"
    );
    assert!(output.contains("  + profile 'test'"), "{output}");
    assert!(!output.contains(".cprof"), "{output}");
    assert!(!destination.config_path().exists());
    assert!(!destination.path.join(".cprof/profiles").exists());
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
