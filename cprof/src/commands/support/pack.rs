use std::path::Path;

use crate::package::pack::PackReport;

pub(crate) fn render_summary(report: &PackReport, output: &Path) {
    println!(
        "Packed {} profile(s) from {} target(s) to '{}'",
        report.profile_count,
        report.target_count,
        output.display()
    );
}
