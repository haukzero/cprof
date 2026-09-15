use std::fs;

use crate::config;
use crate::editor;
use crate::error::Result;
use crate::targets::EXTRA_TARGET_FILE;

const DEFAULT_TEMPLATE: &[u8] = br#"# Example for extra targets
# [[example.resources]]
# key = "settings" # optional, default: filename
# filename = "settings.json" # REQUIRED
# active_path = ".example/settings.json" # REQUIRED
# template = "{
#     \"env\": {},
#     \"name\": \"Example\",
# }" # optional, default: empty
# required = true # optional, default: true
"#;

pub fn run(editor_name: Option<String>) -> Result<()> {
    let editor_cmd = editor::resolve_editor(editor_name)?;
    let repository = config::repository_dir()?;
    let path = repository.join(EXTRA_TARGET_FILE);

    fs::create_dir_all(repository)?;
    if !path.exists() {
        fs::File::create(&path)?;
        fs::write(&path, DEFAULT_TEMPLATE)?;
    }
    editor::open_editor(&editor_cmd, &path)
}
