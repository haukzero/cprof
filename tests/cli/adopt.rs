use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::process::Command;

use crate::support::TestHome;

const CONFIG: &str = "# Keep comments and formatting\nmodel = 'example'\n";
const AUTH: &str = "{ \"example\": \"test credential\" }\n";

fn unmanaged_codex() -> TestHome {
    let home = TestHome::new();
    fs::create_dir(home.path.join(".codex")).unwrap();
    fs::write(home.path.join(".codex/config.toml"), CONFIG).unwrap();
    fs::write(home.path.join(".codex/auth.json"), AUTH).unwrap();
    home
}

fn assert_originals(home: &TestHome) {
    for (filename, content) in [("config.toml", CONFIG), ("auth.json", AUTH)] {
        let path = home.path.join(".codex").join(filename);
        assert!(!path.is_symlink());
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }
}

#[test]
fn adopts_codex_bytes_permissions_and_both_active_entries() {
    let home = unmanaged_codex();
    fs::set_permissions(
        home.path.join(".codex/auth.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let output = home.succeeds(&["codex", "adopt", "personal profile"]);
    assert!(output.contains("Adopted and activated"), "{output}");
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
    assert_eq!(
        fs::metadata(profile.join("auth.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(profile).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(
        home.fails(&["codex", "adopt", "another"])
            .contains("already managed")
    );
    assert!(!home.path.join(".cprof/profiles/codex/another").exists());
}

#[test]
fn adopts_external_links_without_modifying_their_sources() {
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
fn adopts_a_partially_managed_configuration_as_a_whole() {
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
    for case in ["missing", "invalid", "broken", "directory", "fifo"] {
        let home = unmanaged_codex();
        let auth = home.path.join(".codex/auth.json");
        fs::remove_file(&auth).unwrap();
        match case {
            "missing" => {}
            "invalid" => fs::write(&auth, "invalid JSON").unwrap(),
            "broken" => symlink("missing.json", &auth).unwrap(),
            "directory" => fs::create_dir(&auth).unwrap(),
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
        let error = home.fails(&["codex", "adopt", "new"]);
        assert!(error.contains("auth.json"), "{case}: {error}");
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
    assert!(
        home.fails(&["codex", "adopt", "existing"])
            .contains("already exists")
    );
    assert!(
        home.fails(&["codex", "adopt"])
            .contains("Interactive input required")
    );
    assert!(
        home.fails(&["codex", "adopt", "../escape"])
            .contains("Invalid profile name")
    );
    assert_originals(&home);
    assert_eq!(
        fs::read_to_string(existing.join("auth.json")).unwrap(),
        "{}"
    );
}

#[test]
fn adopts_claude_and_external_targets_without_filling_optional_resources() {
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

#[test]
fn commands_share_adoption_advice_and_json_stdout_stays_parseable() {
    let home = unmanaged_codex();
    let mut warnings = Vec::new();
    for args in [
        vec!["codex", "which"],
        vec!["targets"],
        vec!["targets", "--json"],
        vec!["codex", "switch"],
    ] {
        let output = home.run(&args);
        let stderr = String::from_utf8(output.stderr).unwrap();
        let advice = stderr
            .lines()
            .filter(|line| line.contains("cprof codex adopt <name>"))
            .collect::<Vec<_>>();
        assert_eq!(advice.len(), 1, "{args:?}: {stderr}");
        warnings.push(advice[0].to_string());
        if args == ["targets", "--json"] {
            let rows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            let codex = rows
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["name"] == "codex")
                .unwrap();
            assert!(codex["active"].is_null());
        } else if args == ["targets"] {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("(unmanaged)")
            );
        }
    }
    assert!(warnings.iter().all(|warning| warning == &warnings[0]));
    assert_originals(&home);
    home.succeeds(&["codex", "adopt", "local"]);
    for args in [&["codex", "which"][..], &["targets"][..]] {
        assert!(home.run(args).stderr.is_empty());
    }
}

#[test]
fn switch_preserves_unmanaged_files_unless_forced() {
    let home = unmanaged_codex();
    let profile = home.path.join(".cprof/profiles/codex/work");
    fs::create_dir_all(&profile).unwrap();
    fs::write(profile.join("config.toml"), "# work\n").unwrap();
    fs::write(profile.join("auth.json"), "{}").unwrap();
    assert!(
        home.fails(&["codex", "switch", "work"])
            .contains("adopt <name>")
    );
    assert_originals(&home);
    let output = home.run(&["codex", "switch", "work", "--force"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("without saving")
    );
    assert_eq!(home.succeeds(&["codex", "which"]), "work\n");
}

#[test]
fn switch_removes_optional_links_and_preserves_all_entries_on_conflict() {
    let home = TestHome::new();
    home.set_config(
        "[demo]\n[[demo.resources]]\nfilename = 'settings'\nactive_path = '.demo/settings'\n\
         [[demo.resources]]\nfilename = 'optional'\nactive_path = '.demo/optional'\nrequired = false\n",
    );
    let profiles = home.path.join(".cprof/profiles/demo");
    for name in ["full", "minimal"] {
        fs::create_dir_all(profiles.join(name)).unwrap();
        fs::write(profiles.join(name).join("settings"), name).unwrap();
    }
    fs::write(profiles.join("full/optional"), "original optional").unwrap();
    home.succeeds(&["demo", "switch", "full"]);
    let optional = home.path.join(".demo/optional");
    fs::remove_file(&optional).unwrap();
    fs::write(&optional, "unmanaged optional").unwrap();

    assert!(
        home.fails(&["demo", "switch", "minimal"])
            .contains("not managed")
    );
    assert_eq!(
        fs::read_to_string(home.path.join(".demo/settings")).unwrap(),
        "full"
    );
    assert_eq!(fs::read_to_string(&optional).unwrap(), "unmanaged optional");
    assert_eq!(fs::read_dir(home.path.join(".demo")).unwrap().count(), 2);

    home.succeeds(&["demo", "switch", "full", "--force"]);
    home.succeeds(&["demo", "switch", "minimal"]);
    assert!(!optional.exists());
    assert!(!optional.is_symlink());
    assert_eq!(home.succeeds(&["demo", "which"]), "minimal\n");
    assert_eq!(
        fs::read_to_string(profiles.join("full/optional")).unwrap(),
        "original optional"
    );
    assert_eq!(fs::read_dir(home.path.join(".demo")).unwrap().count(), 1);
}

#[test]
fn relative_managed_links_are_recognized_including_dangling_resources() {
    let home = unmanaged_codex();
    home.succeeds(&["codex", "adopt", "local"]);
    for filename in ["config.toml", "auth.json"] {
        let active = home.path.join(".codex").join(filename);
        fs::remove_file(&active).unwrap();
        symlink(format!("../.cprof/profiles/codex/local/{filename}"), active).unwrap();
    }
    assert_eq!(home.succeeds(&["codex", "which"]), "local\n");
    assert!(home.run(&["targets"]).stderr.is_empty());
    assert!(home.succeeds(&["codex", "switch", "local"]).is_empty());
    assert!(
        home.fails(&["codex", "adopt", "again"])
            .contains("already managed")
    );
    fs::remove_file(home.path.join(".cprof/profiles/codex/local/auth.json")).unwrap();
    let output = home.run(&["codex", "which"]);
    let warning = String::from_utf8(output.stderr).unwrap();
    assert!(warning.contains("incomplete"), "{warning}");
    assert!(!warning.contains("adopt"));
    home.succeeds(&["codex", "clean", "--force"]);
    assert!(!home.path.join(".codex/config.toml").is_symlink());
    assert!(!home.path.join(".codex/auth.json").is_symlink());
}

#[test]
fn link_parent_aliases_are_resolved_before_parent_components() {
    let home = unmanaged_codex();
    home.succeeds(&["codex", "adopt", "local"]);
    let outside = home.path.join("outside");
    fs::create_dir_all(outside.join("nested")).unwrap();
    fs::write(outside.join("auth.json"), AUTH).unwrap();
    let local = home.path.join(".cprof/profiles/codex/local");
    symlink(outside.join("nested"), local.join("alias")).unwrap();
    let active = home.path.join(".codex/auth.json");
    fs::remove_file(&active).unwrap();
    symlink(local.join("alias/../auth.json"), active).unwrap();
    let warning = String::from_utf8(home.run(&["codex", "which"]).stderr).unwrap();
    assert!(warning.contains("adopt <name>"), "{warning}");
    assert!(
        home.fails(&["codex", "switch", "local"])
            .contains("not managed")
    );
    assert_eq!(fs::read_to_string(outside.join("auth.json")).unwrap(), AUTH);
}
