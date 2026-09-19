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

fn create_claude_profile(home: &TestHome, name: &str) {
    home.succeeds(&["claude", "create", name, "--editor", "/bin/true"]);
}

#[test]
fn profile_counts_include_incomplete_profiles() {
    let home = TestHome::new();
    create_claude_profile(&home, "complete");
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
    assert_eq!(claude["active"], "complete");
    assert_eq!(
        claude["store_dir"],
        home.path.join(".cprof/profiles/claude").to_str().unwrap()
    );
}

#[test]
fn non_interactive_remove_conflict_fails_before_deleting_any_profiles() {
    let home = TestHome::new();
    create_claude_profile(&home, "active");
    create_claude_profile(&home, "inactive");

    let stderr = home.fails(&["claude", "remove", "inactive", "active"]);

    assert!(stderr.contains("Confirmation required"), "{stderr}");
    assert!(home.path.join(".cprof/profiles/claude/active").is_dir());
    assert!(home.path.join(".cprof/profiles/claude/inactive").is_dir());
}

#[test]
fn forced_remove_deletes_an_active_profile_and_its_link() {
    let home = TestHome::new();
    create_claude_profile(&home, "active");

    home.succeeds(&["claude", "remove", "active", "--force"]);

    assert!(!home.path.join(".cprof/profiles/claude/active").exists());
    assert!(!home.path.join(".claude/settings.json").exists());
}

#[test]
fn non_interactive_remove_without_a_conflict_still_succeeds() {
    let home = TestHome::new();
    create_claude_profile(&home, "active");
    create_claude_profile(&home, "inactive");

    home.succeeds(&["claude", "remove", "inactive"]);

    assert!(home.path.join(".cprof/profiles/claude/active").is_dir());
    assert!(!home.path.join(".cprof/profiles/claude/inactive").exists());
}

#[test]
fn non_interactive_clean_requires_force_and_preserves_profiles() {
    let home = TestHome::new();
    create_claude_profile(&home, "active");

    let stderr = home.fails(&["claude", "clean"]);

    assert!(stderr.contains("Confirmation required"), "{stderr}");
    assert!(home.path.join(".cprof/profiles/claude/active").is_dir());
}

#[test]
fn non_interactive_unpack_conflict_fails_before_commit() {
    let source = TestHome::new();
    create_claude_profile(&source, "existing");
    create_claude_profile(&source, "incoming");
    let package = source.path.join("profiles.pkg");
    source.succeeds(&["claude", "pack", "--save", package.to_str().unwrap()]);

    let destination = TestHome::new();
    create_claude_profile(&destination, "existing");
    let stderr = destination.fails(&["claude", "unpack", "--path", package.to_str().unwrap()]);

    assert!(stderr.contains("Confirmation required"), "{stderr}");
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
fn non_interactive_unpack_without_conflicts_still_succeeds() {
    let source = TestHome::new();
    create_claude_profile(&source, "incoming");
    let package = source.path.join("profiles.pkg");
    source.succeeds(&["claude", "pack", "--save", package.to_str().unwrap()]);

    let destination = TestHome::new();
    destination.succeeds(&["claude", "unpack", "--path", package.to_str().unwrap()]);

    assert!(
        destination
            .path
            .join(".cprof/profiles/claude/incoming")
            .is_dir()
    );
}

#[test]
fn edit_extra_opens_invalid_configs_without_replacing_them() {
    let home = TestHome::new();

    for &(config, _) in INVALID_CONFIGS {
        home.set_config(config);
        home.succeeds(&["edit-extra", "--editor", "/bin/true"]);
        assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
    }
}

#[test]
fn invalid_profile_edit_keeps_every_original_and_removes_temporary_files() {
    let home = TestHome::new();
    home.succeeds(&["codex", "create", "test", "--editor", "/bin/true"]);
    let editor = home.path.join("invalid editor");
    fs::write(
        &editor,
        "#!/bin/sh\ncase \"$1\" in\n  *config.toml*) printf 'model = \\\"changed\\\"\\n' > \"$1\" ;;\n  *) printf '{' > \"$1\" ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();

    let stderr = home.fails(&[
        "codex",
        "edit",
        "test",
        "--editor",
        editor.to_str().unwrap(),
    ]);
    let profile = home.path.join(".cprof/profiles/codex/test");
    assert_eq!(
        fs::read_to_string(profile.join("config.toml")).unwrap(),
        "# Codex configuration\n"
    );
    assert_eq!(
        fs::read_to_string(profile.join("auth.json")).unwrap(),
        "{}\n"
    );
    let temporary_files = fs::read_dir(&profile)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains(".cprof-edit-")
        })
        .collect::<Vec<_>>();
    assert!(temporary_files.is_empty(), "{temporary_files:?}");
    assert!(stderr.contains("original file was kept"), "{stderr}");
}

#[test]
fn profile_edit_drafts_keep_the_original_file_extensions() {
    let home = TestHome::new();
    home.succeeds(&["codex", "create", "test", "--editor", "/bin/true"]);
    let editor = home.path.join("extension checking editor");
    fs::write(
        &editor,
        "#!/bin/sh\ncase \"$1\" in\n  *.toml|*.json) exit 0 ;;\n  *) exit 1 ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();

    home.succeeds(&[
        "codex",
        "edit",
        "test",
        "--editor",
        editor.to_str().unwrap(),
    ]);
}

#[test]
fn explicit_editor_arguments_precede_the_draft_path() {
    let home = TestHome::new();
    let editor = home.path.join("argument checking editor");
    fs::write(
        &editor,
        "#!/bin/sh\n[ \"$1\" = --wait ] && [ \"$2\" = 'two words' ] && [ -f \"$3\" ]\n",
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();

    home.succeeds(&[
        "edit-extra",
        "--editor",
        editor.to_str().unwrap(),
        "--editor-arg=--wait",
        "--editor-arg",
        "two words",
    ]);
}

#[test]
fn visual_takes_priority_and_supports_editor_arguments() {
    let home = TestHome::new();
    let editor = home.path.join("visual editor");
    fs::write(
        &editor,
        "#!/bin/sh\n[ \"$1\" = --visual ] || exit 1\nprintf '[visual]\\n' > \"$2\"\n",
    )
    .unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();
    let visual = format!("\"{}\" --visual", editor.display());

    let output = home
        .command(&["edit-extra"])
        .env("VISUAL", visual)
        .env("EDITOR", "/bin/false")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(home.config_path()).unwrap(),
        "[visual]\n"
    );
}

#[test]
fn malformed_visual_command_is_reported() {
    let home = TestHome::new();

    let output = home
        .command(&["edit-extra"])
        .env("VISUAL", "'unterminated")
        .env("EDITOR", "/bin/true")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "{output:?}");
    assert!(stderr.contains("Invalid editor command"), "{stderr}");
    assert!(stderr.contains("missing closing quote"), "{stderr}");
    assert!(!home.config_path().exists());
}

#[test]
fn invalid_extra_target_edit_keeps_original_and_removes_temporary_file() {
    let home = TestHome::new();
    let original = "[demo]\n";
    home.set_config(original);
    let editor = home.path.join("invalid config editor");
    fs::write(&editor, "#!/bin/sh\nprintf '[broken' > \"$1\"\n").unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();

    let stderr = home.fails(&["edit-extra", "--editor", editor.to_str().unwrap()]);

    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), original);
    let temporary_files = fs::read_dir(home.path.join(".cprof"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains(".cprof-edit-")
        })
        .collect::<Vec<_>>();
    assert!(temporary_files.is_empty(), "{temporary_files:?}");
    assert!(stderr.contains("original file was kept"), "{stderr}");
}

#[test]
fn edit_extra_validates_before_creating_the_config_file() {
    let home = TestHome::new();
    let editor = home.path.join("invalid new config editor");
    fs::write(&editor, "#!/bin/sh\nprintf '[broken' > \"$1\"\n").unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700)).unwrap();

    home.fails(&["edit-extra", "--editor", editor.to_str().unwrap()]);

    assert!(!home.config_path().exists());
    let entries = fs::read_dir(home.path.join(".cprof"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(entries, 0);
}

#[test]
fn edit_extra_creates_the_valid_default_transactionally() {
    let home = TestHome::new();

    home.succeeds(&["edit-extra", "--editor", "/bin/true"]);

    let content = fs::read_to_string(home.config_path()).unwrap();
    assert!(content.contains("Example for extra targets"));
    assert!(
        fs::read_dir(home.path.join(".cprof"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .all(|name| !name.to_string_lossy().contains(".cprof-edit-"))
    );
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
    let stderr = destination.fails(&[
        "unpack",
        "--path",
        source.path.join("cprof.pkg").to_str().unwrap(),
        "--force",
    ]);

    assert!(
        stderr.contains("Unsafe path") || stderr.contains("IO error"),
        "{stderr}"
    );
    assert!(!destination.config_path().exists());
    assert!(!profiles.join("codex/test").exists());
    assert_eq!(
        fs::read_to_string(profiles.join("demo-id")).unwrap(),
        "blocks the target directory"
    );
}
