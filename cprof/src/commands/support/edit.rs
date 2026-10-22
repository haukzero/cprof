use crate::cli::EditorOptions;
use crate::config;
use crate::error::{ProfileError, Result};
use crate::profile::storage;
use crate::targets::{ResourceSpec, TargetSpec, external};
use crate::ui::editor::EditSession;

use super::mutation::{self, MutationSession};

const DEFAULT_TEMPLATE: &[u8] = br#"# Example for extra targets
# [[example.resources]]
# key = "settings" # optional, default: filename
# filename = "settings.json" # REQUIRED
# active_path = ".example/settings.json" # optional, relative to HOME
# absolute_active_path = "/absolute/path/settings.json" # alternative; one is required
# template = "{
#     \"env\": {},
#     \"name\": \"Example\",
# }" # optional, default: empty
# required = true # optional, default: true
"#;

pub(crate) fn profile(
    target: &TargetSpec,
    name: &str,
    filename: Option<&str>,
    editor: EditorOptions,
) -> Result<bool> {
    mutation::target(target, || {
        let resources: Vec<&ResourceSpec> = match filename {
            Some(filename) => vec![target.resource(filename)?],
            None => target.resources.iter().collect(),
        };
        let mut session = EditSession::new(editor.editor, editor.editor_args)?;
        for resource in resources {
            let path = storage::resource_path(target, name, resource)?;
            if !path.exists() {
                if resource.required {
                    return Err(ProfileError::Incomplete(name.to_string()).into());
                }
                continue;
            }
            session.edit(&path, |bytes| resource.validate(bytes))?;
        }
        session.commit()
    })
}

pub(crate) fn extra(editor: EditorOptions) -> Result<()> {
    MutationSession::configuration()?.run(|| {
        let path = config::extra_target_file()?;
        let mut session = EditSession::new(editor.editor, editor.editor_args)?;
        session.edit_or_create(&path, DEFAULT_TEMPLATE, |bytes| {
            let configs = external::parse_external_configs(bytes, &path)?;
            external::validate_environment(&configs)?;
            super::config::warn_redundant_paths(&configs);
            Ok(())
        })?;
        session.commit()?;
        Ok(())
    })
}
