use std::fs;

use crate::support::{TestHome, can_symlink};

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
        for name in ["fresh profile", "existing files"] {
            fs::write(&counter, "").unwrap();
            let output = fixture.run(&[
                id,
                "create",
                name,
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
