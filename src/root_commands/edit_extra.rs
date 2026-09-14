use std::fs;

use crate::config;
use crate::editor;
use crate::error::Result;
use crate::targets::EXTRA_TARGET_FILE;

pub fn run(editor_name: Option<String>) -> Result<()> {
    let editor_cmd = editor::resolve_editor(editor_name)?;
    let repository = config::repository_dir()?;
    let path = repository.join(EXTRA_TARGET_FILE);

    fs::create_dir_all(repository)?;
    if !path.exists() {
        fs::File::create(&path)?;
    }
    editor::open_editor(&editor_cmd, &path)
}
