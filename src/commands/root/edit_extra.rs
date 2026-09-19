use std::fs;

use crate::config;
use crate::editor;
use crate::error::Result;

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

pub fn run(editor_name: Option<String>) -> Result<()> {
    let editor_cmd = editor::resolve_editor(editor_name)?;
    let repository = config::repository_dir()?;
    let path = config::extra_target_file()?;

    fs::create_dir_all(repository)?;
    if !path.exists() {
        fs::File::create(&path)?;
        fs::write(&path, DEFAULT_TEMPLATE)?;
    }
    editor::open_editor(&editor_cmd, &path)
}
