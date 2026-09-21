use crate::cli::EditorOptions;
use crate::config;
use crate::error::Result;
use crate::targets::external;
use crate::ui::editor::EditSession;

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

pub fn run(editor: EditorOptions) -> Result<()> {
    let path = config::extra_target_file()?;
    let mut session = EditSession::new(editor.editor, editor.editor_args)?;
    session.edit_or_create(
        &path,
        DEFAULT_TEMPLATE,
        external::validate_external_configs_content,
    )?;
    session.commit()?;
    Ok(())
}
