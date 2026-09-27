use std::collections::BTreeMap;

use crate::cli;
use crate::config::home;
use crate::error::{AppError, TargetError};

use super::{
    ExternalResourceConfig, ExternalTargetConfig, ExtraTarget, merge_external_configs,
    spec_from_external_config,
};

#[test]
fn external_target_ids_cannot_shadow_root_commands() {
    for name in cli::command_names() {
        let config = ExternalTargetConfig {
            name: "example".to_string(),
            id: Some(name.clone()),
            resources: vec![],
        };
        assert!(matches!(
            spec_from_external_config(&config),
            Err(AppError::Target(TargetError::Conflict(_)))
        ));

        // A table name may match a command when its explicit id does not.
        let config = ExternalTargetConfig {
            name: name.clone(),
            id: Some("custom-target".to_string()),
            resources: vec![],
        };
        assert_eq!(
            spec_from_external_config(&config).unwrap().id,
            "custom-target"
        );
    }
}

#[test]
fn external_defaults_and_templates_are_loaded() {
    let target: ExtraTarget = toml::from_str(
        r#"
            id = "custom-id"
            [[resources]]
            filename = "settings.conf"
            active_path = ".config/example/settings.conf"
            [[resources]]
            key = "auth"
            filename = "auth.data"
            active_path = ".config/example/auth.data"
            template = "hello"
            required = false
        "#,
    )
    .unwrap();

    let target = spec_from_external_config(&target.into_config("custom".to_string())).unwrap();
    assert_eq!(target.id, "custom-id");
    assert_eq!(target.resources[0].key, "settings.conf");
    assert!(target.resources[0].required);
    assert!(target.resources[0].template.is_empty());
    assert_eq!(target.resources[1].key, "auth");
    assert!(!target.resources[1].required);
    assert_eq!(target.resources[1].template, b"hello");
}

#[test]
fn external_configs_merge_resources_and_resolve_conflicts() {
    let local = ExternalTargetConfig {
        name: "demo".to_string(),
        id: None,
        resources: vec![ExternalResourceConfig {
            key: Some("settings".to_string()),
            filename: "old.conf".to_string(),
            active_path: Some(".config/demo/old.conf".to_string()),
            absolute_active_path: None,
            template: None,
            required: None,
        }],
    };
    let packaged = ExternalTargetConfig {
        name: "demo".to_string(),
        id: None,
        resources: vec![
            ExternalResourceConfig {
                key: Some("settings".to_string()),
                filename: "new.conf".to_string(),
                active_path: Some(".config/demo/new.conf".to_string()),
                absolute_active_path: None,
                template: None,
                required: None,
            },
            ExternalResourceConfig {
                key: Some("auth".to_string()),
                filename: "auth.json".to_string(),
                active_path: Some(".config/demo/auth.json".to_string()),
                absolute_active_path: None,
                template: None,
                required: Some(false),
            },
        ],
    };
    let mut configs = BTreeMap::new();
    configs.insert(local.name.clone(), local);
    let merged = merge_external_configs(configs, &[packaged], |_| Ok(false)).unwrap();
    let merged = &merged["demo"];
    assert_eq!(merged.resources.len(), 2);
    assert_eq!(merged.resources[0].filename, "old.conf");
    assert_eq!(merged.resources[1].resolved_key(), "auth");
}

#[test]
fn target_id_conflict_keeps_both_definitions_when_local_wins() {
    let local = ExternalTargetConfig {
        name: "demo".to_string(),
        id: Some("local-id".to_string()),
        resources: vec![],
    };
    let packaged = ExternalTargetConfig {
        name: "demo".to_string(),
        id: None,
        resources: vec![],
    };
    let mut configs = BTreeMap::new();
    configs.insert(local.name.clone(), local);
    let merged = merge_external_configs(configs, &[packaged], |_| Ok(false)).unwrap();
    assert!(
        merged
            .values()
            .any(|config| config.resolved_id() == "local-id")
    );
    assert!(merged.values().any(|config| config.resolved_id() == "demo"));
}

fn resource(
    key: &str,
    filename: &str,
    active_path: Option<&str>,
    absolute_active_path: Option<&str>,
) -> ExternalResourceConfig {
    ExternalResourceConfig {
        key: Some(key.to_string()),
        filename: filename.to_string(),
        active_path: active_path.map(str::to_string),
        absolute_active_path: absolute_active_path.map(str::to_string),
        template: None,
        required: None,
    }
}

fn target(resources: Vec<ExternalResourceConfig>) -> ExternalTargetConfig {
    ExternalTargetConfig {
        name: "demo".to_string(),
        id: None,
        resources,
    }
}

#[test]
fn resource_paths_require_one_safe_active_path() {
    let invalid = [
        resource("r", "settings.json", None, None),
        resource("r", "settings/name.json", Some(".config/name"), None),
        resource("r", "settings.json", Some("../escape"), None),
        resource(
            "r",
            "settings.json",
            Some(".config/name"),
            Some("relative/name"),
        ),
        resource(
            "r",
            "settings.json",
            Some(".config/name"),
            Some("/tmp/other"),
        ),
    ];
    for resource in invalid {
        assert!(spec_from_external_config(&target(vec![resource])).is_err());
    }
}

#[test]
fn absolute_active_path_is_supported_and_same_path_is_accepted() {
    let home = home::dir().unwrap();
    let absolute = home.join(".config/demo/settings.json");
    let config = target(vec![resource(
        "settings",
        "settings.json",
        Some(".config/demo/settings.json"),
        Some(absolute.to_str().unwrap()),
    )]);
    let spec = spec_from_external_config(&config).unwrap();
    assert_eq!(
        spec.resources[0].active_path.as_deref(),
        Some(".config/demo/settings.json")
    );
    assert_eq!(
        spec.resources[0].absolute_active_path.as_deref(),
        Some(absolute.to_str().unwrap())
    );
}

#[test]
fn duplicate_filename_and_active_path_are_rejected() {
    let duplicate_filename = target(vec![
        resource("one", "same.json", Some(".config/demo/one.json"), None),
        resource("two", "same.json", Some(".config/demo/two.json"), None),
    ]);
    assert!(spec_from_external_config(&duplicate_filename).is_err());

    let duplicate_active_path = target(vec![
        resource("one", "one.json", Some(".config/demo/same.json"), None),
        resource("two", "two.json", Some(".config/demo/same.json"), None),
    ]);
    assert!(spec_from_external_config(&duplicate_active_path).is_err());
}

#[test]
fn target_and_resource_components_are_rejected() {
    for id in ["", ".", "..", "a/b", "a\\b", "C:\\tmp"] {
        let config = ExternalTargetConfig {
            name: "demo".to_string(),
            id: Some(id.to_string()),
            resources: vec![],
        };
        assert!(spec_from_external_config(&config).is_err(), "{id:?}");
    }
    let invalid_filename = target(vec![resource(
        "r",
        "../settings.json",
        Some(".config/demo/settings.json"),
        None,
    )]);
    assert!(spec_from_external_config(&invalid_filename).is_err());

    for invalid in [
        resource(
            "",
            "settings.json",
            Some(".config/demo/settings.json"),
            None,
        ),
        resource("r", "", Some(".config/demo/settings.json"), None),
        resource("r", "CON.json", Some(".config/demo/settings.json"), None),
        resource("r", "settings.json", Some(""), None),
    ] {
        assert!(spec_from_external_config(&target(vec![invalid])).is_err());
    }
}
