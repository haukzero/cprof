use crate::cli::EditorOptions;
use crate::commands::support::edit;
use crate::error::Result;

#[cprof_macros::command(no_retry)]
pub fn run(editor: EditorOptions) -> Result<()> {
    edit::extra(editor)
}
