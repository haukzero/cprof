use crate::activation::{self, Status};
use crate::error::Result;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec) -> Result<()> {
    match activation::status(target)? {
        Status::Active(name) => println!("{}", name),
        Status::NoFiles => style::warning("No active profile"),
        Status::Partial => style::warning("Active profile is incomplete or partially linked"),
        Status::Mixed => style::warning("Different resources point to different profiles"),
        Status::Unmanaged => style::warning("Active paths contain unmanaged files or links"),
    }
    Ok(())
}
