use crate::commands::warn_adopt;
use crate::error::Result;
use crate::profile::activation::{self, Status};
use crate::targets::TargetSpec;
use crate::ui::style;

pub fn run(target: &TargetSpec) -> Result<()> {
    let status = activation::status(target)?;
    warn_adopt(&target.id, &status);
    match status {
        Status::Active(name) => println!("{}", name),
        Status::NoFiles => style::warning("No active profile"),
        Status::Partial => style::warning("Active profile is incomplete or partially linked"),
        Status::Mixed => style::warning("Different resources point to different profiles"),
        Status::Unmanaged => {}
    }
    Ok(())
}
