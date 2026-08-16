use std::fs;
use std::path::PathBuf;

use cprof::config;
use cprof::profile;
use serial_test::serial;

/// Generate a unique profile name for testing
fn unique_name(suffix: &str) -> String {
    format!("test_{}_{}", std::process::id(), suffix)
}

/// Save the current symlink target (if any) and return it
fn save_symlink() -> Option<PathBuf> {
    let link = config::settings_link().ok()?;
    fs::read_link(&link).ok()
}

/// Restore the symlink to its original target
fn restore_symlink(target: Option<PathBuf>) {
    if let Some(target) = target {
        let link = config::settings_link().unwrap();
        // Remove current symlink if it exists
        let _ = fs::remove_file(&link);
        // Recreate symlink
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&target, &link);
        #[cfg(windows)]
        let _ = std::os::windows::fs::symlink_file(&target, &link);
    }
}

#[test]
#[serial]
fn test_profile_lifecycle() {
    let original = save_symlink();
    let name = unique_name("lifecycle");

    // Create a profile
    let content = r#"{"env": {}, "permissions": {"allow": []}}"#;
    let path = profile::create_profile(&name, content).unwrap();
    assert!(path.exists());
    assert_eq!(fs::read_to_string(&path).unwrap(), content);

    // List profiles - find ours
    let profiles = profile::list_profiles().unwrap();
    let found = profiles.iter().find(|p| p.name == name);
    assert!(found.is_some());
    assert!(!found.unwrap().active);

    // Switch to profile
    let was_active = profile::switch_profile(&name).unwrap();
    assert!(!was_active);

    // Verify active
    let active = profile::get_active_name().unwrap();
    assert_eq!(active.as_deref(), Some(name.as_str()));

    // Switch again - should return true (already active)
    let was_active = profile::switch_profile(&name).unwrap();
    assert!(was_active);

    // Remove profile
    let was_active = profile::remove_profile(&name).unwrap();
    assert!(was_active);

    // Verify removed
    let profiles = profile::list_profiles().unwrap();
    assert!(!profiles.iter().any(|p| p.name == name));

    // Restore original symlink
    restore_symlink(original);
}

#[test]
fn test_duplicate_profile_name() {
    let name = unique_name("duplicate");

    let content = r#"{"env": {}}"#;
    profile::create_profile(&name, content).unwrap();

    // Try to create with same name
    let result = profile::create_profile(&name, content);
    assert!(result.is_err());

    // Cleanup
    let _ = profile::remove_profile(&name);
}

#[test]
fn test_nonexistent_profile() {
    let name = unique_name("ghost_nonexistent");

    // Try to switch to non-existent profile
    let result = profile::switch_profile(&name);
    assert!(result.is_err());

    // Try to remove non-existent profile
    let result = profile::remove_profile(&name);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_multiple_profiles() {
    let original = save_symlink();
    let name_a = unique_name("multi_a");
    let name_b = unique_name("multi_b");
    let name_c = unique_name("multi_c");

    let content = r#"{"env": {}}"#;
    profile::create_profile(&name_a, content).unwrap();
    profile::create_profile(&name_b, content).unwrap();
    profile::create_profile(&name_c, content).unwrap();

    let profiles = profile::list_profiles().unwrap();
    let our_profiles: Vec<_> = profiles
        .iter()
        .filter(|p| {
            p.name
                .starts_with(&format!("test_{}_multi", std::process::id()))
        })
        .collect();
    assert_eq!(our_profiles.len(), 3);

    // Switch to b
    profile::switch_profile(&name_b).unwrap();
    let active = profile::get_active_name().unwrap();
    assert_eq!(active.as_deref(), Some(name_b.as_str()));

    // Cleanup
    let _ = profile::remove_profile(&name_a);
    let _ = profile::remove_profile(&name_b);
    let _ = profile::remove_profile(&name_c);

    // Restore original symlink
    restore_symlink(original);
}

#[test]
fn test_read_profile() {
    let name = unique_name("reader");

    let content = r#"{"env": {"KEY": "value"}}"#;
    profile::create_profile(&name, content).unwrap();

    let read_content = profile::read_profile(&name).unwrap();
    assert_eq!(read_content, content);

    // Cleanup
    let _ = profile::remove_profile(&name);
}

#[test]
fn test_read_nonexistent_profile() {
    let name = unique_name("read_ghost");

    let result = profile::read_profile(&name);
    assert!(result.is_err());
}

#[test]
fn test_config_paths() {
    let profiles = config::profiles_dir().unwrap();
    assert!(profiles.to_string_lossy().contains(".claude-profiles"));

    let settings = config::settings_link().unwrap();
    assert!(settings.to_string_lossy().contains("settings.json"));
}

#[test]
#[serial]
fn test_error_when_regular_file() {
    let original = save_symlink();
    let link = config::settings_link().unwrap();

    // Replace symlink with a regular file
    let _ = fs::remove_file(&link);
    fs::write(&link, r#"{"env":{}}"#).unwrap();

    // get_active_name returns None, get_settings_status returns NotManaged
    assert_eq!(profile::get_active_name().unwrap(), None);
    assert_eq!(
        profile::get_settings_status().unwrap(),
        profile::SettingsStatus::NotManaged
    );

    // Restore
    let _ = fs::remove_file(&link);
    restore_symlink(original);
}

#[test]
#[serial]
fn test_error_when_external_symlink() {
    let original = save_symlink();
    let link = config::settings_link().unwrap();

    // Create symlink to external path
    let _ = fs::remove_file(&link);
    let external = std::env::temp_dir().join("external_cprof_test.json");
    fs::write(&external, r#"{"env":{}}"#).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&external, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&external, &link).unwrap();

    // get_active_name returns None, get_settings_status returns ExternalSymlink
    assert_eq!(profile::get_active_name().unwrap(), None);
    match profile::get_settings_status().unwrap() {
        profile::SettingsStatus::ExternalSymlink(path) => {
            assert!(path.contains("external_cprof_test.json"));
        }
        other => panic!("Expected ExternalSymlink, got: {:?}", other),
    }

    // Cleanup
    let _ = fs::remove_file(&link);
    let _ = fs::remove_file(&external);
    restore_symlink(original);
}

mod pack_tests {
    use super::*;
    use cprof::commands::{pack, unpack};

    #[test]
    #[serial]
    fn test_pack_unpack_roundtrip() {
        let name_alpha = unique_name("pack_alpha");
        let name_beta = unique_name("pack_beta");

        // Create some profiles
        let content1 = r#"{"env": {"A": "1"}}"#;
        let content2 = r#"{"env": {"B": "2"}}"#;
        profile::create_profile(&name_alpha, content1).unwrap();
        profile::create_profile(&name_beta, content2).unwrap();

        // Pack
        let pkg_path = config::profiles_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("test_{}.pkg", std::process::id()));
        pack::run(Some(pkg_path.to_string_lossy().to_string())).unwrap();
        assert!(pkg_path.exists());

        // Remove profiles
        profile::remove_profile(&name_alpha).unwrap();
        profile::remove_profile(&name_beta).unwrap();

        // Unpack
        unpack::run(Some(pkg_path.to_string_lossy().to_string()), false).unwrap();

        // Verify profiles restored
        let profiles = profile::list_profiles().unwrap();
        assert!(profiles.iter().any(|p| p.name == name_alpha));
        assert!(profiles.iter().any(|p| p.name == name_beta));

        // Verify content
        let restored1 = profile::read_profile(&name_alpha).unwrap();
        assert_eq!(restored1, content1);
        let restored2 = profile::read_profile(&name_beta).unwrap();
        assert_eq!(restored2, content2);

        // Cleanup
        let _ = profile::remove_profile(&name_alpha);
        let _ = profile::remove_profile(&name_beta);
        let _ = fs::remove_file(&pkg_path);
    }

    #[test]
    fn test_invalid_package_magic() {
        let pkg_path = config::profiles_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("bad_{}.pkg", std::process::id()));
        fs::write(&pkg_path, b"NOTCPKG").unwrap();

        let result = unpack::run(Some(pkg_path.to_string_lossy().to_string()), false);
        assert!(result.is_err());

        let _ = fs::remove_file(&pkg_path);
    }

    #[test]
    #[serial]
    fn test_corrupted_checksum() {
        let name = unique_name("corrupt");

        // Create a profile and pack it
        profile::create_profile(&name, r#"{"env": {}}"#).unwrap();
        let pkg_path = config::profiles_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("corrupt_{}.pkg", std::process::id()));
        pack::run(Some(pkg_path.to_string_lossy().to_string())).unwrap();

        // Corrupt the last byte
        let mut data = fs::read(&pkg_path).unwrap();
        let last = data.len() - 1;
        data[last] = data[last].wrapping_add(1);
        fs::write(&pkg_path, data).unwrap();

        // Try to unpack
        let result = unpack::run(Some(pkg_path.to_string_lossy().to_string()), false);
        assert!(result.is_err());

        // Cleanup
        let _ = profile::remove_profile(&name);
        let _ = fs::remove_file(&pkg_path);
    }

    #[test]
    #[serial]
    fn test_unpack_force_overwrite() {
        let name = unique_name("force");
        let original = save_symlink();

        // Create a profile with original content
        let original_content = r#"{"env": {"KEY": "original"}}"#;
        profile::create_profile(&name, original_content).unwrap();

        // Pack it
        let pkg_path = config::profiles_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!("force_{}.pkg", std::process::id()));
        pack::run(Some(pkg_path.to_string_lossy().to_string())).unwrap();

        // Modify the profile with different content
        let modified_content = r#"{"env": {"KEY": "modified"}}"#;
        let profile_path = config::profile_settings(&name).unwrap();
        fs::write(&profile_path, modified_content).unwrap();

        // Verify modified content
        assert_eq!(fs::read_to_string(&profile_path).unwrap(), modified_content);

        // Unpack with force=true - should overwrite
        unpack::run(Some(pkg_path.to_string_lossy().to_string()), true).unwrap();

        // Verify content restored from package
        let restored = profile::read_profile(&name).unwrap();
        assert_eq!(restored, original_content);

        // Cleanup
        let _ = profile::remove_profile(&name);
        let _ = fs::remove_file(&pkg_path);
        restore_symlink(original);
    }
}
