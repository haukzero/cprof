use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::Command;

use crate::support::{
    AUTH, CONFIG, TestHome, assert_originals, can_symlink, symlink, unmanaged_codex,
};

#[test]
fn adopts_codex_bytes_permissions_and_both_active_entries() {
    let home = unmanaged_codex();
    if !can_symlink() {
        home.fails(&["codex", "adopt", "personal profile"], "1314");
        assert_originals(&home);
        assert!(
            !home
                .path
                .join(".cprof/profiles/codex/personal profile")
                .exists()
        );
        return;
    }
    #[cfg(unix)]
    fs::set_permissions(
        home.path.join(".codex/auth.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let output = home.succeeds(&["codex", "adopt", "personal profile"]);
    assert!(
        output.contains("Adopted and activated") || cfg!(windows) && output.is_empty(),
        "{output}"
    );
    assert_eq!(home.succeeds(&["codex", "which"]), "personal profile\n");
    let profile = home.path.join(".cprof/profiles/codex/personal profile");
    for (filename, content) in [("config.toml", CONFIG), ("auth.json", AUTH)] {
        let stored = profile.join(filename);
        assert!(!stored.is_symlink());
        assert_eq!(fs::read_to_string(&stored).unwrap(), content);
        assert_eq!(
            fs::read_link(home.path.join(".codex").join(filename)).unwrap(),
            stored
        );
    }
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(profile.join("auth.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(profile).unwrap().permissions().mode() & 0o777,
        0o700
    );
    home.fails(&["codex", "adopt", "another"], "already managed");
    assert!(!home.path.join(".cprof/profiles/codex/another").exists());
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn adopts_external_links_without_modifying_their_sources() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = unmanaged_codex();
    let outside = tempfile::tempdir().unwrap();
    for (filename, content) in [("config.toml", CONFIG), ("auth.json", AUTH)] {
        let source = outside.path().join(filename);
        fs::write(&source, content).unwrap();
        let active = home.path.join(".codex").join(filename);
        fs::remove_file(&active).unwrap();
        symlink(&source, active).unwrap();
    }
    home.succeeds(&["codex", "adopt", "imported"]);
    for (filename, content) in [("config.toml", CONFIG), ("auth.json", AUTH)] {
        assert_eq!(
            fs::read_to_string(outside.path().join(filename)).unwrap(),
            content
        );
    }
    fs::write(outside.path().join("auth.json"), "{}").unwrap();
    assert_eq!(
        fs::read_to_string(home.path.join(".codex/auth.json")).unwrap(),
        AUTH
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn adopts_a_partially_managed_configuration_as_a_whole() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = unmanaged_codex();
    let old = home.path.join(".cprof/profiles/codex/old");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("config.toml"), CONFIG).unwrap();
    fs::write(old.join("auth.json"), "{}").unwrap();
    fs::remove_file(home.path.join(".codex/config.toml")).unwrap();
    symlink(
        old.join("config.toml"),
        home.path.join(".codex/config.toml"),
    )
    .unwrap();
    home.succeeds(&["codex", "adopt", "current"]);
    assert_eq!(home.succeeds(&["codex", "which"]), "current\n");
    assert_eq!(fs::read_to_string(old.join("config.toml")).unwrap(), CONFIG);
    assert_eq!(fs::read_to_string(old.join("auth.json")).unwrap(), "{}");
    assert_eq!(
        fs::read_to_string(home.path.join(".codex/auth.json")).unwrap(),
        AUTH
    );
}

#[test]
fn adoption_rejects_missing_invalid_and_broken_resources_before_writing() {
    let mut cases = vec!["missing", "invalid", "directory"];
    if can_symlink() {
        cases.push("broken");
    }
    #[cfg(unix)]
    cases.push("fifo");
    for case in cases {
        let home = unmanaged_codex();
        let auth = home.path.join(".codex/auth.json");
        fs::remove_file(&auth).unwrap();
        match case {
            "missing" => {}
            "invalid" => fs::write(&auth, "invalid JSON").unwrap(),
            "broken" => symlink("missing.json", &auth).unwrap(),
            "directory" => fs::create_dir(&auth).unwrap(),
            #[cfg(unix)]
            "fifo" => {
                assert!(
                    Command::new("mkfifo")
                        .arg(&auth)
                        .status()
                        .unwrap()
                        .success()
                );
            }
            _ => unreachable!(),
        }
        home.fails(&["codex", "adopt", "new"], "auth.json");
        assert!(!home.path.join(".cprof/profiles/codex/new").exists());
        assert!(!home.path.join(".codex/config.toml").is_symlink());
        assert_eq!(
            fs::read_to_string(home.path.join(".codex/config.toml")).unwrap(),
            CONFIG
        );
    }
}

#[test]
fn adoption_rejects_existing_names_and_noninteractive_missing_names() {
    let home = unmanaged_codex();
    let existing = home.path.join(".cprof/profiles/codex/existing");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("auth.json"), "{}").unwrap();
    home.fails(&["codex", "adopt", "existing"], "already exists");
    home.fails(&["codex", "adopt"], "Interactive input required");
    home.fails(&["codex", "adopt", "../escape"], "Invalid profile name");
    assert_originals(&home);
    assert_eq!(
        fs::read_to_string(existing.join("auth.json")).unwrap(),
        "{}"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn adopts_claude_and_external_targets_without_filling_optional_resources() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    fs::create_dir(home.path.join(".claude")).unwrap();
    fs::write(home.path.join(".claude/settings.json"), "{}\n").unwrap();
    home.succeeds(&["claude", "adopt", "local"]);
    assert_eq!(home.succeeds(&["claude", "which"]), "local\n");

    let outside = tempfile::tempdir().unwrap();
    let active = outside.path().join("settings.conf");
    fs::write(&active, "arbitrary configuration\n").unwrap();
    home.set_config(&format!(
        "[demo]\n[[demo.resources]]\nfilename = 'settings.conf'\nabsolute_active_path = '{}'\n\
         [[demo.resources]]\nfilename = 'optional.conf'\nactive_path = '.demo/optional.conf'\n\
         required = false\ntemplate = 'must not be created'\n",
        active.display()
    ));
    home.succeeds(&["demo", "adopt", "local"]);
    assert_eq!(home.succeeds(&["demo", "which"]), "local\n");
    assert!(active.is_symlink());
    assert_eq!(
        fs::read_to_string(active).unwrap(),
        "arbitrary configuration\n"
    );
    assert!(!home.path.join(".demo/optional.conf").exists());
    assert!(
        !home
            .path
            .join(".cprof/profiles/demo/local/optional.conf")
            .exists()
    );
}
