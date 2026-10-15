use std::fs;

use crate::support::{AUTH, CONFIG, INVALID_CONFIGS, TestHome, assert_failure};

#[cprof_macros::windows_elevation]
#[test]
fn edit_active_selects_the_current_profile_and_warns_for_an_explicit_name() {
    for selection in [
        vec!["-a"],
        vec!["--active"],
        vec!["work", "-a"],
        vec!["work", "--active"],
    ] {
        let home = TestHome::new();
        let profile = home.codex_profile("work");
        let other = home.codex_profile("other");
        home.succeeds(&["codex", "switch", "work"]);
        let editor = home.editor(
            "case \"$1\" in\n*.toml) printf \"model = 'edited'\\n\" > \"$1\" ;;\n*) printf '{\"edited\":true}' > \"$1\" ;;\nesac\n",
            "if /i \"%~x1\"==\".toml\" (\n>\"%~1\" echo model = 'edited'\n) else (\n>\"%~1\" echo {\"edited\":true}\n)\n",
        );

        let mut args = vec!["codex", "edit", "--editor", editor.to_str().unwrap()];
        args.extend_from_slice(&selection);
        let output = home.run(&args);
        assert!(output.status.success(), "{output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if selection.contains(&"work") {
            assert!(
                stderr.contains("warning: Profile 'work' is already selected by --active"),
                "{stderr}"
            );
        } else {
            assert!(stderr.is_empty(), "{stderr}");
        }
        assert!(String::from_utf8_lossy(&output.stdout).contains("Updated profile 'work'"));
        for (filename, expected) in [
            ("config.toml", "model = 'edited'"),
            ("auth.json", "{\"edited\":true}"),
        ] {
            assert_eq!(
                fs::read_to_string(profile.join(filename)).unwrap().trim(),
                expected
            );
            let active = home.path.join(".codex").join(filename);
            assert!(active.is_symlink());
            assert_eq!(fs::read_to_string(active).unwrap().trim(), expected);
        }
        assert_eq!(
            fs::read_to_string(other.join("config.toml")).unwrap(),
            CONFIG
        );
        assert_eq!(fs::read_to_string(other.join("auth.json")).unwrap(), AUTH);
        assert_eq!(home.succeeds(&["codex", "which"]).trim(), "work");
    }
}

#[test]
fn edit_active_requires_an_active_profile_before_launching_the_editor() {
    for state in ["empty", "inactive", "unmanaged"] {
        let home = TestHome::new();
        if state != "empty" {
            home.codex_profile("work");
        }
        if state == "unmanaged" {
            fs::create_dir(home.path.join(".codex")).unwrap();
            fs::write(home.path.join(".codex/config.toml"), CONFIG).unwrap();
            fs::write(home.path.join(".codex/auth.json"), AUTH).unwrap();
        }
        let editor = home.editor("touch editor-started\n", "type nul > editor-started\n");
        for flag in ["-a", "--active"] {
            for name in [None, Some("work")] {
                let mut args = vec!["codex", "edit", flag, "--editor", editor.to_str().unwrap()];
                args.extend(name);
                let expected = if state == "empty" && name.is_some() {
                    "Profile 'work' not found"
                } else {
                    "No active profile for target 'codex'"
                };
                home.fails(&args, expected);
                assert!(!home.path.join("editor-started").exists());
            }
        }
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn edit_active_rejects_a_different_name_before_launching_the_editor() {
    let home = TestHome::new();
    let active = home.codex_profile("work");
    let other = home.codex_profile("other");
    home.succeeds(&["codex", "switch", "work"]);
    let editor = home.editor("touch editor-started\n", "type nul > editor-started\n");
    for flag in ["-a", "--active"] {
        for (name, expected) in [
            ("other", "Profile 'other' is not the active profile"),
            ("missing", "Profile 'missing' not found"),
            ("../escape", "Invalid profile name '../escape'"),
        ] {
            let output = home.run(&[
                "codex",
                "edit",
                name,
                flag,
                "--editor",
                editor.to_str().unwrap(),
            ]);
            assert_failure(&output, expected);
            if name == "other" {
                assert_failure(&output, "active: 'work'");
            }
            assert!(!String::from_utf8_lossy(&output.stderr).contains("warning:"));
            assert!(!home.path.join("editor-started").exists());
        }
    }
    for profile in [active, other] {
        assert_eq!(
            fs::read_to_string(profile.join("config.toml")).unwrap(),
            CONFIG
        );
        assert_eq!(fs::read_to_string(profile.join("auth.json")).unwrap(), AUTH);
        assert_eq!(fs::read_dir(profile).unwrap().count(), 2);
    }
}

#[test]
fn edit_validates_explicit_names_with_or_without_active() {
    let home = TestHome::new();
    for flag in [None, Some("-a"), Some("--active")] {
        for (name, expected) in [
            ("missing", "Profile 'missing' not found"),
            ("../escape", "Invalid profile name '../escape'"),
        ] {
            let mut args = vec!["codex", "edit", name];
            args.extend(flag);
            home.fails(&args, expected);
        }
    }
}

#[test]
fn edit_extra_opens_invalid_configs_without_replacing_them() {
    let home = TestHome::new();
    for &(config, _) in INVALID_CONFIGS {
        home.set_config(config);
        home.succeeds(&["edit-extra"]);
        assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
    }
}

#[test]
fn invalid_second_resource_keeps_every_original_and_removes_drafts() {
    let home = TestHome::new();
    let profile = home.codex_profile("test");
    let editor = home.editor(
        "case \"$1\" in\n*.toml) printf \"model = 'changed'\\n\" > \"$1\" ;;\n*) printf '{' > \"$1\" ;;\nesac\n",
        "if /i \"%~x1\"==\".toml\" (\n>\"%~1\" echo model = 'changed'\n) else (\n>\"%~1\" echo {\n)\n",
    );
    home.fails(
        &[
            "codex",
            "edit",
            "test",
            "--editor",
            editor.to_str().unwrap(),
        ],
        "original file was kept",
    );
    assert_eq!(
        fs::read_to_string(profile.join("config.toml")).unwrap(),
        CONFIG
    );
    assert_eq!(fs::read_to_string(profile.join("auth.json")).unwrap(), AUTH);
    assert_eq!(fs::read_dir(profile).unwrap().count(), 2);
}

#[test]
fn profile_edit_drafts_keep_the_original_file_extensions() {
    let home = TestHome::new();
    home.codex_profile("test");
    let editor = home.editor(
        "case \"$1\" in\n*.toml|*.json) exit 0 ;;\n*) exit 1 ;;\nesac\n",
        "if /i \"%~x1\"==\".toml\" exit /b 0\nif /i \"%~x1\"==\".json\" exit /b 0\nexit /b 1\n",
    );
    home.succeeds(&[
        "codex",
        "edit",
        "test",
        "--editor",
        editor.to_str().unwrap(),
    ]);
}

#[test]
fn explicit_editor_overrides_environment_and_preserves_arguments() {
    let home = TestHome::new();
    let editor = home.editor(
        "[ \"$1\" = --wait ] && [ \"$2\" = 'two words' ] && [ -f \"$3\" ]\n",
        "if not \"%~1\"==\"--wait\" exit /b 1\nif not \"%~2\"==\"two words\" exit /b 1\nif not exist \"%~3\" exit /b 1\nexit /b 0\n",
    );
    let output = home
        .command(&[
            "edit-extra",
            "--editor",
            editor.to_str().unwrap(),
            "--editor-arg=--wait",
            "--editor-arg",
            "two words",
        ])
        .env("CPROF_EDITOR", "'unterminated")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(home.config_path().is_file());
}

#[test]
fn editor_environment_priority_preserves_command_arguments() {
    for name in ["CPROF_EDITOR", "VISUAL"] {
        let home = TestHome::new();
        let editor = home.editor(
            "[ \"$1\" = 'two words' ] || exit 1\nprintf '[edited]\\n' > \"$2\"\n",
            "if not \"%~1\"==\"two words\" exit /b 1\n>\"%~2\" echo [edited]\n",
        );
        let command = format!(
            "{} 'two words'",
            shell_words::quote(editor.to_str().unwrap())
        );
        let output = home
            .command(&["edit-extra"])
            .env("VISUAL", "must-not-launch-this-editor")
            .env("EDITOR", "must-not-launch-this-editor")
            .env(name, command)
            .output()
            .unwrap();
        assert!(output.status.success(), "{name}: {output:?}");
        assert_eq!(
            fs::read_to_string(home.config_path()).unwrap().trim(),
            "[edited]"
        );
    }
}

#[test]
fn blank_editor_environment_values_fall_back() {
    for blank in ["", " \t\n "] {
        for fallback in ["VISUAL", "EDITOR"] {
            let home = TestHome::new();
            let editor = home.editor(
                "printf '[fallback]\\n' > \"$1\"\n",
                ">\"%~1\" echo [fallback]\n",
            );
            let output = home
                .command(&["edit-extra"])
                .env("CPROF_EDITOR", blank)
                .env("VISUAL", blank)
                .env("EDITOR", "must-not-launch-this-editor")
                .env(
                    fallback,
                    shell_words::quote(editor.to_str().unwrap()).as_ref(),
                )
                .output()
                .unwrap();
            assert!(output.status.success(), "{fallback}: {output:?}");
            assert_eq!(
                fs::read_to_string(home.config_path()).unwrap().trim(),
                "[fallback]"
            );
        }
    }
}

#[test]
fn malformed_editor_environment_commands_are_reported() {
    for name in ["CPROF_EDITOR", "VISUAL", "EDITOR"] {
        let home = TestHome::new();
        let output = home
            .command(&["edit-extra"])
            .env(name, "'unterminated")
            .output()
            .unwrap();
        assert_failure(&output, "Invalid editor command");
        assert_failure(&output, "missing closing quote");
        assert!(!home.config_path().exists());
    }
}

#[test]
fn invalid_extra_target_edits_preserve_existing_or_absent_config() {
    for original in [None, Some("[demo]\n")] {
        let home = TestHome::new();
        if let Some(content) = original {
            home.set_config(content);
        }
        let editor = home.editor("printf '[broken' > \"$1\"\n", ">\"%~1\" echo [broken\n");
        home.fails(
            &["edit-extra", "--editor", editor.to_str().unwrap()],
            "original file was kept",
        );
        assert_eq!(
            fs::read_to_string(home.config_path()).ok().as_deref(),
            original
        );
        assert_eq!(
            fs::read_dir(home.path.join(".cprof")).unwrap().count(),
            usize::from(original.is_some())
        );
    }
}

#[cprof_macros::windows_elevation]
#[test]
fn profile_create_resumes_an_interrupted_draft() {
    let home = TestHome::new();
    home.codex_profile("source");
    let profile = home.path.join(".cprof/profiles/codex/recovered");
    fs::create_dir_all(&profile).unwrap();
    fs::write(profile.join("config.toml"), CONFIG).unwrap();
    fs::write(profile.join("auth.json"), AUTH).unwrap();
    fs::write(
        profile.join(".config.cprof-edit.toml"),
        "model = 'recovered'\n",
    )
    .unwrap();

    let editor = home.editor("exit 0\n", "exit /b 0\n");
    home.succeeds(&[
        "codex",
        "create",
        "recovered",
        "-c",
        "source",
        "--editor",
        editor.to_str().unwrap(),
    ]);

    assert_eq!(
        fs::read_to_string(profile.join("config.toml")).unwrap(),
        "model = 'recovered'\n"
    );
    assert_eq!(fs::read_to_string(profile.join("auth.json")).unwrap(), AUTH);
    assert!(!profile.join(".config.cprof-edit.toml").exists());
}

#[test]
fn edit_extra_creates_the_valid_default_transactionally() {
    let home = TestHome::new();
    home.succeeds(&["edit-extra"]);
    let content = fs::read_to_string(home.config_path()).unwrap();
    assert!(content.contains("Example for extra targets"));
    assert_eq!(fs::read_dir(home.path.join(".cprof")).unwrap().count(), 1);
}
