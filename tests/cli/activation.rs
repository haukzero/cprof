use std::fs;
use std::path::PathBuf;

use crate::support::{
    AUTH, CONFIG, TestHome, assert_originals, can_symlink, symlink, unmanaged_codex,
};

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn codex_reports_partial_and_mixed_links() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    let first = home.codex_profile("first");
    home.codex_profile("second");
    home.succeeds(&["codex", "switch", "first"]);
    assert_eq!(home.succeeds(&["codex", "which"]), "first\n");
    let auth = home.path.join(".codex/auth.json");
    fs::remove_file(&auth).unwrap();
    let partial = home.run(&["codex", "which"]);
    assert!(partial.status.success());
    assert!(String::from_utf8_lossy(&partial.stderr).contains("partially linked"));
    home.succeeds(&["codex", "switch", "second"]);
    fs::remove_file(&auth).unwrap();
    symlink(first.join("auth.json"), &auth).unwrap();
    let mixed = home.run(&["codex", "which"]);
    assert!(mixed.status.success());
    assert!(String::from_utf8_lossy(&mixed.stderr).contains("Different resources"));
    assert!(home.path.join(".codex/config.toml").is_symlink());
}

#[test]
#[cfg(unix)]
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
    home.fails(&["codex", "switch", "work"], "adopt <name>");
    assert_originals(&home);
    let output = home.run(&["codex", "switch", "work", "--force"]);
    if !can_symlink() {
        crate::support::assert_failure(&output, "1314");
        assert_originals(&home);
        return;
    }
    assert!(output.status.success());
    #[cfg(unix)]
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("without saving")
    );
    assert_eq!(home.succeeds(&["codex", "which"]), "work\n");
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn switch_removes_optional_links_and_preserves_all_entries_on_conflict() {
    assert!(can_symlink(), "symbolic-link privileges are required");
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

    home.fails(&["demo", "switch", "minimal"], "not managed");
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
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn relative_managed_links_are_recognized_including_dangling_resources() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = unmanaged_codex();
    home.succeeds(&["codex", "adopt", "local"]);
    let relative: PathBuf = ["..", ".cprof", "profiles", "codex", "local"]
        .iter()
        .collect();
    for (filename, content) in [("config.toml", CONFIG), ("auth.json", AUTH)] {
        let active = home.path.join(".codex").join(filename);
        fs::remove_file(&active).unwrap();
        symlink(relative.join(filename), &active).unwrap();
        assert_eq!(fs::read_to_string(active).unwrap(), content);
    }
    assert_eq!(home.succeeds(&["codex", "which"]), "local\n");
    assert!(home.run(&["targets"]).stderr.is_empty());
    assert!(home.succeeds(&["codex", "switch", "local"]).is_empty());
    home.fails(&["codex", "adopt", "again"], "already managed");
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
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn link_parent_aliases_follow_native_path_resolution() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = unmanaged_codex();
    home.succeeds(&["codex", "adopt", "local"]);
    let outside = home.path.join("outside");
    fs::create_dir_all(outside.join("nested")).unwrap();
    fs::write(outside.join("auth.json"), AUTH).unwrap();
    let local = home.path.join(".cprof/profiles/codex/local");
    symlink(outside.join("nested"), local.join("alias")).unwrap();
    let active = home.path.join(".codex/auth.json");
    fs::remove_file(&active).unwrap();
    symlink(local.join("alias").join("..").join("auth.json"), &active).unwrap();
    let resolved = fs::canonicalize(&active).unwrap();
    #[cfg(unix)]
    {
        // Unix follows the alias before resolving its parent component.
        assert_eq!(
            resolved,
            fs::canonicalize(outside.join("auth.json")).unwrap()
        );
        let warning = String::from_utf8(home.run(&["codex", "which"]).stderr).unwrap();
        assert!(warning.contains("adopt <name>"), "{warning}");
        home.fails(&["codex", "switch", "local"], "not managed");
    }
    #[cfg(windows)]
    {
        // Win32 normalizes alias\.. in this absolute target before following links.
        assert_eq!(resolved, fs::canonicalize(local.join("auth.json")).unwrap());
        assert_eq!(home.succeeds(&["codex", "which"]), "local\n");
        home.succeeds(&["codex", "switch", "local"]);
    }
    assert_eq!(fs::read_to_string(outside.join("auth.json")).unwrap(), AUTH);
}
