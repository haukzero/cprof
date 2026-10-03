use std::path::Path;

use crate::error::Result;
use crate::package::{self, pack};
use crate::targets::TargetRepository;
use crate::ui::tree::TreeStyle;

mod selection;

pub(crate) use selection::Scope;

fn branch_style(ascii: bool) -> TreeStyle {
    if ascii || std::env::var_os("CPROF_ASCII").is_some_and(|value| value == "1") {
        TreeStyle::Ascii
    } else {
        TreeStyle::Unicode
    }
}

pub(crate) fn run(
    targets: &TargetRepository,
    scope: Scope<'_>,
    save: Option<String>,
    select: Option<Vec<String>>,
    ascii: bool,
) -> Result<()> {
    let output_path = save.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let output = Path::new(&output_path);
    let selected = selection::resolve(targets, scope, select, branch_style(ascii))?;
    let report = pack::create(targets, &selected, output)?;
    println!(
        "Packed {} profile(s) from {} target(s) to '{}'",
        report.profile_count,
        report.target_count,
        output.display()
    );
    Ok(())
}
