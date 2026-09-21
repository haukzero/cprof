use std::fs;

use crate::support::{AUTH, CONFIG, INVALID_CONFIGS, TestHome, assert_failure};

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
fn explicit_editor_arguments_precede_the_draft_path() {
    let home = TestHome::new();
    let editor = home.editor(
        "[ \"$1\" = --wait ] && [ \"$2\" = 'two words' ] && [ -f \"$3\" ]\n",
        "if not \"%~1\"==\"--wait\" exit /b 1\nif not \"%~2\"==\"two words\" exit /b 1\nif not exist \"%~3\" exit /b 1\nexit /b 0\n",
    );
    home.succeeds(&[
        "edit-extra",
        "--editor",
        editor.to_str().unwrap(),
        "--editor-arg=--wait",
        "--editor-arg",
        "two words",
    ]);
    assert!(home.config_path().is_file());
}

#[test]
fn visual_takes_priority_and_supports_editor_arguments() {
    let home = TestHome::new();
    let editor = home.editor(
        "[ \"$1\" = --visual ] || exit 1\nprintf '[visual]\\n' > \"$2\"\n",
        "if not \"%~1\"==\"--visual\" exit /b 1\n>\"%~2\" echo [visual]\n",
    );
    let visual = format!("{} --visual", shell_words::quote(editor.to_str().unwrap()));
    let output = home
        .command(&["edit-extra"])
        .env("VISUAL", visual)
        .env("EDITOR", "must-not-launch-this-editor")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(home.config_path()).unwrap().trim(),
        "[visual]"
    );
}

#[test]
fn malformed_visual_command_is_reported() {
    let home = TestHome::new();
    let output = home
        .command(&["edit-extra"])
        .env("VISUAL", "'unterminated")
        .output()
        .unwrap();
    assert_failure(&output, "Invalid editor command");
    assert_failure(&output, "missing closing quote");
    assert!(!home.config_path().exists());
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

#[test]
fn edit_extra_creates_the_valid_default_transactionally() {
    let home = TestHome::new();
    home.succeeds(&["edit-extra"]);
    let content = fs::read_to_string(home.config_path()).unwrap();
    assert!(content.contains("Example for extra targets"));
    assert_eq!(fs::read_dir(home.path.join(".cprof")).unwrap().count(), 1);
}
