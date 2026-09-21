#![cfg(windows)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn cprof(home: &Path, args: &[&str]) -> Output {
    // Isolate the real home and exercise the child path without opening UAC.
    Command::new(env!("CARGO_BIN_EXE_cprof"))
        .arg("--elevated-home")
        .arg(home)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

#[test]
fn builtin_and_extra_profiles_preserve_edits_across_activation_failures() {
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join("home with spaces");
    fs::create_dir_all(home.join(".cprof")).unwrap();
    let absolute = directory.path().join("outside/b.txt");
    fs::write(
        home.join(".cprof/extra-target.toml"),
        format!(
            "[[desktop.resources]]\nfilename = 'a.txt'\nactive_path = 'Desktop/a.txt'\n\
             [[hello.resources]]\nfilename = 'b.txt'\nabsolute_active_path = '{}'\n\
             [[hi.resources]]\nfilename = 'c.txt'\nactive_path = 'hi/c.txt'\n\
             [[hihi.resources]]\nfilename = 'd.txt'\nactive_path = '.hihi/txt'\n",
            absolute.display()
        ),
    )
    .unwrap();

    let probe = home.join("probe");
    let can_link = match std::os::windows::fs::symlink_file(home.join("missing"), &probe) {
        Ok(()) => {
            fs::remove_file(probe).unwrap();
            true
        }
        Err(error) => {
            assert_eq!(error.raw_os_error(), Some(1314));
            false
        }
    };
    // A batch editor keeps the real process boundary without PowerShell startup.
    let editor = home.join("editor.cmd");
    let counter = home.join("editor-count");
    fs::write(
        &editor,
        r#"@echo off
>>"%~1" echo edit
if /i "%~x2"==".toml" (
    >"%~2" echo model = 'edited'
) else (
    >"%~2" echo {"edited":true}
)
"#,
    )
    .unwrap();
    let cases = [
        (
            "claude",
            vec![("settings.json", home.join(".claude/settings.json"))],
        ),
        (
            "codex",
            vec![
                ("config.toml", home.join(".codex/config.toml")),
                ("auth.json", home.join(".codex/auth.json")),
            ],
        ),
        ("desktop", vec![("a.txt", home.join("Desktop/a.txt"))]),
        ("hello", vec![("b.txt", absolute)]),
        ("hi", vec![("c.txt", home.join("hi/c.txt"))]),
        ("hihi", vec![("d.txt", home.join(".hihi/txt"))]),
    ];
    for (id, resources) in cases {
        for name in ["fresh profile", "existing files"] {
            fs::write(&counter, "").unwrap();
            let output = cprof(
                &home,
                &[
                    id,
                    "create",
                    name,
                    "--editor",
                    editor.to_str().unwrap(),
                    "--editor-arg",
                    counter.to_str().unwrap(),
                ],
            );
            assert_eq!(
                output.status.success(),
                can_link || name == "existing files",
                "{id}: {output:?}"
            );
            assert_eq!(
                fs::read_to_string(&counter).unwrap().lines().count(),
                resources.len()
            );
            let profile = home.join(".cprof/profiles").join(id).join(name);
            assert_eq!(fs::read_dir(&profile).unwrap().count(), resources.len());
            for (filename, active) in &resources {
                let saved = fs::read(profile.join(filename)).unwrap();
                assert!(String::from_utf8_lossy(&saved).contains("edited"));
                if name == "fresh profile" {
                    assert_eq!(active.is_symlink(), can_link);
                    assert_eq!(active.exists(), can_link);
                    if can_link {
                        assert_eq!(fs::read(active).unwrap(), saved);
                        fs::remove_file(active).unwrap();
                    }
                    // The same target with an existing active file must create
                    // another profile without attempting automatic activation.
                    fs::write(active, &saved).unwrap();
                } else {
                    assert!(!active.is_symlink());
                    assert_eq!(fs::read(active).unwrap(), saved);
                }
            }
        }

        let output = cprof(&home, &[id, "adopt", "imported profile"]);
        assert_eq!(output.status.success(), can_link, "{id}: {output:?}");
        let imported = home
            .join(".cprof/profiles")
            .join(id)
            .join("imported profile");
        assert_eq!(imported.exists(), can_link);
        let old_name = if can_link {
            "imported profile"
        } else {
            "fresh profile"
        };
        let output = cprof(&home, &[id, "rename", old_name, "renamed profile"]);
        assert!(output.status.success(), "{id}: {output:?}");
        let profiles = home.join(".cprof/profiles").join(id);
        assert!(!profiles.join(old_name).exists());
        for (filename, active) in &resources {
            assert_eq!(active.is_symlink(), can_link);
            assert!(String::from_utf8_lossy(&fs::read(active).unwrap()).contains("edited"));
            let renamed = profiles.join("renamed profile").join(filename);
            assert!(String::from_utf8_lossy(&fs::read(&renamed).unwrap()).contains("edited"));
            if can_link {
                assert_eq!(
                    fs::canonicalize(active).unwrap(),
                    fs::canonicalize(renamed).unwrap()
                );
            }
        }
    }
}
