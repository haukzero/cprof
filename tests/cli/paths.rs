use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read, Write};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::support::{TestHome, can_symlink, symlink};

fn resource(key: &str, filename: &str, active_path: &str) -> toml::Table {
    toml::toml! {
        key = key
        filename = filename
        active_path = active_path
    }
}

fn config(resources: &[toml::Table]) -> String {
    let mut target = toml::toml! { id = "demo-id" };
    target.insert(
        "resources".into(),
        toml::Value::Array(resources.iter().cloned().map(toml::Value::Table).collect()),
    );
    toml::to_string(&BTreeMap::from([("demo", target)])).unwrap()
}

fn rejects_config(home: &TestHome, config: &str, expected: &str) {
    home.set_config(config);
    home.fails(&["clean", "--force", "--extra-toml"], expected);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
    assert!(!home.path.join(".cprof/profiles").exists());
}

/// Mutate only the manifest, keeping ZIP framing and the checksum valid.
fn tamper_manifest(package: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    const HEADER_LEN: usize = 18;
    const CHECKSUM_LEN: usize = 32;
    assert_eq!(old.len(), new.len());
    assert!(!old.is_empty());
    let archive_len = u64::from_le_bytes(package[10..HEADER_LEN].try_into().unwrap()) as usize;
    assert_eq!(package.len(), HEADER_LEN + archive_len + CHECKSUM_LEN);

    let mut archive =
        ZipArchive::new(Cursor::new(&package[HEADER_LEN..HEADER_LEN + archive_len])).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut replacements = 0;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        if file.name() == "manifest.bin" {
            let offsets: Vec<_> = contents
                .windows(old.len())
                .enumerate()
                .filter_map(|(offset, bytes)| (bytes == old).then_some(offset))
                .collect();
            replacements += offsets.len();
            for offset in offsets {
                contents[offset..offset + new.len()].copy_from_slice(new);
            }
        }
        writer.start_file(file.name(), options).unwrap();
        writer.write_all(&contents).unwrap();
    }
    assert!(
        replacements > 0,
        "selected bytes must occur in the manifest"
    );
    let archive = writer.finish().unwrap().into_inner();
    let mut framed = package[..10].to_vec();
    framed.extend_from_slice(&(archive.len() as u64).to_le_bytes());
    framed.extend_from_slice(&archive);
    framed.extend_from_slice(&Sha256::digest(&framed));
    framed
}

#[test]
fn local_path_validation_cannot_be_bypassed_with_force() {
    let home = TestHome::new();
    for (input, expected) in [
        (
            "[demo]\nid = '../escape'\n".to_string(),
            "Invalid target id",
        ),
        ("[demo]\nid = 'C:escape'\n".to_string(), "Invalid target id"),
        ("[\"../escape\"]\n".to_string(), "Invalid target id"),
        (
            config(&[resource("settings", "../file", ".demo/file")]),
            "Invalid filename",
        ),
        (
            config(&[resource("settings", "file", "../escape")]),
            "Invalid active path",
        ),
        (
            config(&[resource("settings", "file", r"C:\escape")]),
            "Invalid active path",
        ),
    ] {
        rejects_config(&home, &input, expected);
    }
}

#[test]
fn packaged_paths_are_validated_before_forced_unpack_writes() {
    let source = TestHome::new();
    let input = config(&[resource("settings", "file.conf", "safe/file")]);
    let profile = source.path.join(".cprof/profiles/demo-id/test");
    fs::create_dir_all(&profile).unwrap();
    fs::write(profile.join("file.conf"), "packaged content").unwrap();
    let package = source.pack_external_target(&input, "demo-id");
    let original = fs::read(&package).unwrap();
    let mutated = source.path.join("mutated.pkg");

    // Prove the fixture rebuilds a valid package before testing malicious fields.
    fs::write(
        &mutated,
        tamper_manifest(&original, b"safe/file", b"safe/file"),
    )
    .unwrap();
    let destination = TestHome::new();
    destination.succeeds(&["unpack", "--path", mutated.to_str().unwrap(), "--force"]);
    assert_eq!(
        fs::read(
            destination
                .path
                .join(".cprof/profiles/demo-id/test/file.conf")
        )
        .unwrap(),
        b"packaged content"
    );

    for (old, new, expected) in [
        (&b"demo-id"[..], &b"../evil"[..], "Invalid target id"),
        (&b"file.conf"[..], &b"../secret"[..], "Invalid filename"),
        (&b"safe/file"[..], &b"../escape"[..], "Invalid active path"),
    ] {
        fs::write(&mutated, tamper_manifest(&original, old, new)).unwrap();
        let destination = TestHome::new();
        destination.fails(
            &["unpack", "--path", mutated.to_str().unwrap(), "--force"],
            expected,
        );
        assert!(!destination.config_path().exists());
        assert!(!destination.path.join(".cprof/profiles").exists());
    }
}

#[test]
fn non_current_manifest_versions_are_rejected() {
    let source = TestHome::new();
    let package = source.pack_external_target("[demo]\n", "demo");
    let original = fs::read(package).unwrap();
    let mutated = source.path.join("version.pkg");

    for version in [0, 1, 2, 3, 255] {
        let replacement = [b'C', b'P', b'M', b'F', version];
        fs::write(
            &mutated,
            tamper_manifest(&original, b"CPMF\x04", &replacement),
        )
        .unwrap();
        let destination = TestHome::new();
        destination.fails(
            &["unpack", "--path", mutated.to_str().unwrap(), "--force"],
            "Unsupported package manifest version",
        );
        assert!(!destination.config_path().exists());
        assert!(!destination.path.join(".cprof/profiles").exists());
    }
}

#[test]
fn different_resource_keys_cannot_share_files_or_active_paths() {
    let home = TestHome::new();
    for (first, second, expected) in [
        (
            resource("first", "same", ".demo/first"),
            resource("second", "same", ".demo/second"),
            "duplicate filename",
        ),
        (
            resource("first", "first", ".demo/same"),
            resource("second", "second", ".demo/same"),
            "same active path",
        ),
        (
            resource("first", "first", ".demo/./same"),
            resource("second", "second", ".demo/same"),
            "same active path",
        ),
    ] {
        rejects_config(&home, &config(&[first, second]), expected);
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn parent_symlink_aliases_cannot_hide_duplicate_active_paths() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    fs::create_dir(home.path.join(".demo")).unwrap();
    symlink(home.path.join(".demo"), home.path.join(".alias")).unwrap();
    rejects_config(
        &home,
        &config(&[
            resource("first", "first", ".demo/settings"),
            resource("second", "second", ".alias/settings"),
        ]),
        "same active path",
    );
}

#[test]
fn merging_valid_definitions_cannot_introduce_resource_collisions() {
    for (filename, active_path, expected) in [
        ("first.conf", ".demo/second", "duplicate filename"),
        ("second.conf", ".demo/first", "same active path"),
    ] {
        let source = TestHome::new();
        let incoming = config(&[resource("second", filename, active_path)]);
        let profile = source.path.join(".cprof/profiles/demo-id/test");
        fs::create_dir_all(&profile).unwrap();
        fs::write(profile.join(filename), "incoming").unwrap();
        let package = source.pack_external_target(&incoming, "demo-id");

        let destination = TestHome::new();
        let local = config(&[resource("first", "first.conf", ".demo/first")]);
        destination.set_config(&local);
        destination.fails(
            &["unpack", "--path", package.to_str().unwrap(), "--force"],
            expected,
        );
        assert_eq!(
            fs::read_to_string(destination.config_path()).unwrap(),
            local
        );
        assert!(!destination.path.join(".cprof/profiles").exists());
    }
}

#[test]
fn dual_active_paths_warn_when_equal_and_error_when_different() {
    let home = TestHome::new();
    let mut settings = resource("settings", "settings.json", ".demo/settings.json");
    settings.insert(
        "absolute_active_path".into(),
        home.path
            .join(".demo/settings.json")
            .to_str()
            .unwrap()
            .into(),
    );
    home.set_config(&config(&[settings.clone()]));
    let output = home.run(&["targets"]);
    assert!(output.status.success(), "{output:?}");
    let warning = String::from_utf8(output.stderr).unwrap();
    assert!(
        warning.contains("active_path and absolute_active_path to the same path"),
        "{warning}"
    );

    settings.insert(
        "absolute_active_path".into(),
        home.path
            .join(".other/settings.json")
            .to_str()
            .unwrap()
            .into(),
    );
    rejects_config(
        &home,
        &config(&[settings]),
        "conflicting active_path and absolute_active_path",
    );
}

#[test]
fn absolute_active_path_survives_activation_and_package_roundtrip() {
    let home = TestHome::new();
    let outside = tempfile::tempdir().unwrap();
    let active = outside.path().join("settings.json");
    let mut settings = resource("settings", "settings.json", ".unused/settings.json");
    settings.remove("active_path");
    settings.insert(
        "absolute_active_path".into(),
        active.to_str().unwrap().into(),
    );
    home.set_config(&config(&[settings]));

    let profile = home.write_profile(
        "demo-id",
        "profile",
        &[("settings.json", "packaged content")],
    );
    if can_symlink() {
        home.succeeds(&["demo-id", "switch", "profile"]);
        assert!(active.is_symlink());
        assert_eq!(
            fs::canonicalize(&active).unwrap(),
            fs::canonicalize(profile.join("settings.json")).unwrap()
        );
    }
    home.succeeds(&["pack"]);

    let destination = TestHome::new();
    destination.succeeds(&[
        "unpack",
        "--path",
        home.path.join("cprof.pkg").to_str().unwrap(),
        "--force",
    ]);
    let restored: toml::Value =
        toml::from_str(&fs::read_to_string(destination.config_path()).unwrap()).unwrap();
    let restored = &restored["demo"]["resources"][0];
    assert_eq!(restored["absolute_active_path"].as_str(), active.to_str());
    assert!(restored.get("active_path").is_none());
    assert!(
        destination
            .path
            .join(".cprof/profiles/demo-id/profile/settings.json")
            .is_file()
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn forced_switch_rejects_parent_symlink_escape_with_missing_descendants() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), home.path.join("redirect")).unwrap();
    home.set_config(&config(&[resource(
        "settings",
        "settings.json",
        "redirect/missing/settings.json",
    )]));

    home.fails(&["demo-id", "switch", "test", "--force"], "Unsafe path");
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(!home.path.join(".cprof/profiles").exists());
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn repository_and_config_symlinks_cannot_escape_home() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    for link_path in [".cprof", ".cprof/extra-target.toml"] {
        let home = TestHome::new();
        let outside = tempfile::tempdir().unwrap();
        let external_config = outside.path().join("extra-target.toml");
        fs::write(&external_config, "[demo]\n").unwrap();
        let link = home.path.join(link_path);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        let source = if link_path == ".cprof" {
            outside.path()
        } else {
            external_config.as_path()
        };
        symlink(source, &link).unwrap();

        home.fails(&["targets"], "Unsafe path");
        assert_eq!(fs::read_to_string(&external_config).unwrap(), "[demo]\n");
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "requires symbolic-link privileges; run with --include-ignored"
)]
fn stored_resources_cannot_be_symlink_aliases() {
    assert!(can_symlink(), "symbolic-link privileges are required");
    let home = TestHome::new();
    let outside = tempfile::tempdir().unwrap();
    let source = outside.path().join("settings.json");
    fs::write(&source, "outside").unwrap();
    home.set_config(&config(&[resource(
        "settings",
        "settings.json",
        ".demo/settings.json",
    )]));
    let profile = home.path.join(".cprof/profiles/demo-id/test");
    fs::create_dir_all(&profile).unwrap();
    symlink(&source, profile.join("settings.json")).unwrap();

    home.fails(&["demo-id", "where", "test"], "Unsafe path");
    assert_eq!(fs::read_to_string(&source).unwrap(), "outside");
}
