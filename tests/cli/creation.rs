use std::fs;

use crate::support::{TestHome, can_symlink};

#[test]
fn named_create_requires_source_selection_without_copy_from() {
    for with_source in [false, true] {
        let home = TestHome::new();
        if with_source {
            home.claude_profile("source");
            home.codex_profile("source");
        }

        for target in ["claude", "codex"] {
            home.fails(
                &[target, "create", "new profile"],
                "Interactive input required",
            );
            assert!(
                !home
                    .path
                    .join(".cprof/profiles")
                    .join(target)
                    .join("new profile")
                    .exists()
            );
            assert!(!home.path.join(format!(".{target}")).exists());
        }
    }
}

#[test]
fn named_create_rejects_existing_profiles() {
    let home = TestHome::new();
    home.claude_profile("source");
    let profile = home.claude_profile("existing");

    home.fails(
        &["claude", "create", "existing", "-c", "source"],
        "already exists",
    );

    assert_eq!(
        fs::read_to_string(profile.join("settings.json")).unwrap(),
        "{}\n"
    );
    assert!(!home.path.join(".claude").exists());
}

#[test]
fn create_saves_once_and_only_activates_when_no_files_exist() {
    let fixture = TestHome::new();
    let home = &fixture.path;
    fs::create_dir_all(home.join(".cprof")).unwrap();
    let absolute = home.parent().unwrap().join("outside/b.txt");
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

    let can_link = can_symlink();
    let counter = home.join("editor-count");
    let editor = fixture.editor(
        "printf 'edit\\n' >> \"$1\"\ncase \"$2\" in\n*.toml) printf \"model = 'edited'\\n\" > \"$2\" ;;\n*) printf '{\"edited\":true}\\n' > \"$2\" ;;\nesac\n",
        ">>\"%~1\" echo edit\nif /i \"%~x2\"==\".toml\" (\n>\"%~2\" echo model = 'edited'\n) else (\n>\"%~2\" echo {\"edited\":true}\n)\n",
    );
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
        let source_resources = resources
            .iter()
            .map(|(filename, _)| {
                (
                    *filename,
                    if filename.ends_with(".json") {
                        "{}\n"
                    } else {
                        ""
                    },
                )
            })
            .collect::<Vec<_>>();
        fixture.write_profile(id, "source", &source_resources);
        for name in ["fresh profile", "existing files"] {
            fs::write(&counter, "").unwrap();
            let output = fixture.run(&[
                id,
                "create",
                name,
                "-c",
                "source",
                "--editor",
                editor.to_str().unwrap(),
                "--editor-arg",
                counter.to_str().unwrap(),
            ]);
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
    }
}
