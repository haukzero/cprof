pub mod root;
pub mod target;

use std::path::Path;

use crate::elevate;
use crate::error::Result;
use crate::package::pack::PackReport;
use crate::package::unpack::{UnpackInteraction, UnpackReport};
use crate::profile::activation;
use crate::ui::{prompt, style};

/// Keep migration advice at the presentation layer, including JSON commands'
/// stderr, rather than emitting it from activation queries used by the library.
pub(crate) fn warn_adopt(target_id: &str, status: &activation::Status) {
    if matches!(status, activation::Status::Unmanaged) && !elevate::is_elevated_child() {
        style::warning(format!(
            "Active paths for '{target_id}' contain unmanaged files or links; \
             use `cprof {target_id} adopt <name>` to preserve and manage the current configuration"
        ));
    }
}

fn report_pack(report: &PackReport, output: &Path) {
    println!(
        "Packed {} profile(s) from {} target(s) to '{}'",
        report.profile_count,
        report.target_count,
        output.display()
    );
}

fn report_unpack(report: &UnpackReport) {
    for target in &report.targets {
        for name in &target.profiles {
            println!("Unpacked '{name}'");
        }
        println!(
            "{}",
            style::heading(&format!("Unpacked target '{}'", target.target))
        );
    }
}

/// Command-layer interaction policy shared by root and target unpack commands.
struct UnpackPrompt {
    force: bool,
}

impl UnpackInteraction for UnpackPrompt {
    fn use_packaged_config(&mut self, conflict: &str) -> Result<bool> {
        Ok(self.force || prompt::confirm(conflict)?)
    }

    fn begin_target(&mut self, target: &str) {
        println!(
            "{}",
            style::heading(&format!("Unpacking target '{target}':"))
        );
    }

    fn overwrite_profile(&mut self, name: &str) -> Result<bool> {
        style::warning(format!("Profile '{name}' already exists - conflict!"));
        let overwrite = self.force || prompt::confirm(&format!("Overwrite '{name}' ?"))?;
        if !overwrite {
            println!("Skipped '{name}'");
        }
        Ok(overwrite)
    }
}
