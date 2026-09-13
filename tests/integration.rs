use std::fs;
use std::path::PathBuf;

use clap::Parser;
use cprof::activation;
use cprof::cli::{TargetCli, TargetCommand};
use cprof::config;
use cprof::package;
use cprof::profile;
use cprof::targets;
use serial_test::serial;

fn unique_name(suffix: &str) -> String {
    format!("test_{}_{}", std::process::id(), suffix)
}

fn claude() -> &'static targets::TargetSpec {
    targets::get("claude").unwrap()
}
fn codex() -> &'static targets::TargetSpec {
    targets::get("codex").unwrap()
}

fn cleanup(target: &'static targets::TargetSpec, name: &str) {
    let _ = activation::remove_profile_links(target, name);
    let _ = profile::delete(target, name);
}

struct LinkBackup {
    links: Vec<(PathBuf, Option<PathBuf>)>,
}

impl LinkBackup {
    fn new(target: &'static targets::TargetSpec) -> Self {
        let links: Vec<(PathBuf, Option<PathBuf>)> = target
            .resources
            .iter()
            .map(|resource| {
                let path = config::active_resource(target, resource).unwrap();
                (path.clone(), fs::read_link(path).ok())
            })
            .collect();
        for (path, _) in links.iter() {
            let is_symlink = fs::symlink_metadata(path)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false);
            if is_symlink {
                let _ = fs::remove_file(path);
            }
        }
        Self { links }
    }
}

impl Drop for LinkBackup {
    fn drop(&mut self) {
        for (path, original) in &self.links {
            let _ = fs::remove_file(path);
            if let Some(target) = original {
                #[cfg(unix)]
                let _ = std::os::unix::fs::symlink(target, path);
                #[cfg(windows)]
                let _ = std::os::windows::fs::symlink_file(target, path);
            }
        }
    }
}

#[test]
#[serial]
fn claude_profile_lifecycle() {
    let target = claude();
    let _links = LinkBackup::new(target);
    let name = unique_name("claude_lifecycle");
    profile::create(target, &name, None).unwrap();
    assert!(profile::is_complete(target, &name).unwrap());
    assert!(
        config::profile_resource(target, &name, &target.resources[0])
            .unwrap()
            .exists()
    );
    assert!(activation::active_name(target).unwrap().is_none());

    assert!(!activation::switch(target, &name, true).unwrap());
    assert_eq!(
        activation::active_name(target).unwrap().as_deref(),
        Some(name.as_str())
    );
    assert!(activation::switch(target, &name, false).unwrap());
    activation::remove_profile_links(target, &name).unwrap();
    profile::delete(target, &name).unwrap();
    assert!(!profile::exists(target, &name).unwrap());
}

#[test]
#[serial]
fn codex_profile_has_two_resources_and_switches_together() {
    let target = codex();
    let _links = LinkBackup::new(target);
    let name = unique_name("codex");
    profile::create(target, &name, None).unwrap();
    let resources = profile::read(target, &name).unwrap();
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].spec.key, "config");
    assert_eq!(resources[1].spec.key, "auth");

    activation::switch(target, &name, true).unwrap();
    assert_eq!(
        activation::active_name(target).unwrap().as_deref(),
        Some(name.as_str())
    );
    for resource in target.resources {
        let link = config::active_resource(target, resource).unwrap();
        assert!(link.is_symlink());
        assert!(link.exists());
    }
    cleanup(target, &name);
}

#[test]
#[serial]
fn codex_reports_partial_and_mixed_links() {
    let target = codex();
    let _links = LinkBackup::new(target);
    let first = unique_name("first");
    let second = unique_name("second");
    profile::create(target, &first, None).unwrap();
    profile::create(target, &second, None).unwrap();
    activation::switch(target, &first, true).unwrap();

    let auth_link = config::active_resource(target, &target.resources[1]).unwrap();
    fs::remove_file(&auth_link).unwrap();
    assert_eq!(
        activation::status(target).unwrap(),
        activation::Status::Partial
    );

    activation::switch(target, &second, true).unwrap();
    let config_link = config::active_resource(target, &target.resources[0]).unwrap();
    let auth_link = config::active_resource(target, &target.resources[1]).unwrap();
    fs::remove_file(&auth_link).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        config::profile_resource(target, &first, &target.resources[1]).unwrap(),
        &auth_link,
    )
    .unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(
        config::profile_resource(target, &first, &target.resources[1]).unwrap(),
        &auth_link,
    )
    .unwrap();
    assert!(config_link.exists());
    assert_eq!(
        activation::status(target).unwrap(),
        activation::Status::Mixed
    );

    cleanup(target, &first);
    cleanup(target, &second);
}

#[test]
fn profile_name_validation() {
    let target = claude();
    assert!(profile::create(target, "../escape", None).is_err());
    assert!(profile::create(target, "", None).is_err());
}

#[test]
#[serial]
fn copied_profile_preserves_resources() {
    let target = codex();
    let source = unique_name("copy_source");
    let destination = unique_name("copy_destination");
    profile::create(target, &source, None).unwrap();
    let config_path = config::profile_resource(target, &source, &target.resources[0]).unwrap();
    fs::write(&config_path, "model = \"gpt-5\"\nsource=\"test_source\"\n").unwrap();

    profile::create(target, &destination, Some(&source)).unwrap();

    for resource in target.resources {
        let source_path = config::profile_resource(target, &source, resource).unwrap();
        let destination_path = config::profile_resource(target, &destination, resource).unwrap();
        assert_eq!(
            fs::read(source_path).unwrap(),
            fs::read(destination_path).unwrap()
        );
    }
    cleanup(target, &source);
    cleanup(target, &destination);
}

#[test]
#[serial]
fn copied_profile_can_start_incomplete() {
    let target = codex();
    let source = unique_name("incomplete_copy_source");
    let destination = unique_name("incomplete_copy_destination");
    profile::create(target, &source, None).unwrap();
    let missing_resource = config::profile_resource(target, &source, &target.resources[1]).unwrap();
    fs::remove_file(missing_resource).unwrap();

    profile::create(target, &destination, Some(&source)).unwrap();

    assert!(!profile::is_complete(target, &destination).unwrap());
    cleanup(target, &source);
    cleanup(target, &destination);
}

#[test]
fn copy_from_requires_an_existing_profile() {
    let target = claude();
    let destination = unique_name("copy_missing_destination");
    let missing = unique_name("copy_missing_source");
    let error = profile::create(target, &destination, Some(&missing)).unwrap_err();

    assert!(matches!(error, cprof::error::AppError::ProfileNotFound(name) if name == missing));
    assert!(!profile::exists(target, &destination).unwrap());
}

#[test]
fn create_accepts_copy_from_option() {
    let cli =
        TargetCli::try_parse_from(["cprof", "create", "new-profile", "-c", "source"]).unwrap();

    assert!(matches!(
        cli.command,
        TargetCommand::Create {
            name: Some(name),
            copy_from: Some(source),
            editor: None,
        } if name == "new-profile" && source == "source"
    ));
}

#[test]
fn package_roundtrip_preserves_resources() {
    let target = codex();
    let first = unique_name("package_first");
    let second = unique_name("package_second");
    profile::create(target, &first, None).unwrap();
    profile::create(target, &second, None).unwrap();
    let entries = vec![
        package::PackageProfile::new(first.clone(), profile::read(target, &first).unwrap()),
        package::PackageProfile::new(second.clone(), profile::read(target, &second).unwrap()),
    ];
    let data = package::encode(&[package::TargetPackage::new("codex", entries)]).unwrap();
    let decoded = package::decode(&data).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].target, "codex");
    assert_eq!(decoded[0].profiles.len(), 2);
    assert_eq!(decoded[0].profiles[0].resources.len(), 2);
    cleanup(target, &first);
    cleanup(target, &second);
}

#[test]
fn package_detects_corruption() {
    let target = claude();
    let name = unique_name("corrupt");
    profile::create(target, &name, None).unwrap();
    let entry = package::PackageProfile::new(name.clone(), profile::read(target, &name).unwrap());
    let mut data = package::encode(&[package::TargetPackage::new("claude", vec![entry])]).unwrap();
    let last = data.len() - 1;
    data[last] ^= 1;
    assert!(package::decode(&data).is_err());
    cleanup(target, &name);
}

#[test]
fn package_roundtrip_supports_all_targets() {
    let claude_target = claude();
    let codex_target = codex();
    let claude_name = unique_name("package_claude");
    let codex_name = unique_name("package_codex");
    profile::create(claude_target, &claude_name, None).unwrap();
    profile::create(codex_target, &codex_name, None).unwrap();

    let package = package::encode(&[
        package::TargetPackage::new(
            "claude",
            vec![package::PackageProfile::new(
                claude_name.clone(),
                profile::read(claude_target, &claude_name).unwrap(),
            )],
        ),
        package::TargetPackage::new(
            "codex",
            vec![package::PackageProfile::new(
                codex_name.clone(),
                profile::read(codex_target, &codex_name).unwrap(),
            )],
        ),
    ])
    .unwrap();
    let decoded = package::decode(&package).unwrap();

    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].target, "claude");
    assert_eq!(decoded[1].target, "codex");
    cleanup(claude_target, &claude_name);
    cleanup(codex_target, &codex_name);
}

#[test]
fn config_paths_are_centralized() {
    let target = codex();
    let root = config::profiles_root().unwrap();
    assert!(root.ends_with(PathBuf::from(".cprof/profiles")));
    assert!(config::profiles_dir(target).unwrap().ends_with("codex"));
}
