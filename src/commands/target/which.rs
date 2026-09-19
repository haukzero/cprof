use crate::activation::{self, Status};
use crate::error::Result;
use crate::style;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec) -> Result<()> {
    match activation::status(target)? {
        Status::Active(name) => println!("{}", name),
        Status::NoFiles => println!("{}", style::warning("No active profile")),
        Status::Partial => println!(
            "{}",
            style::warning("Active profile is incomplete or partially linked")
        ),
        Status::Mixed => println!(
            "{}",
            style::warning("Different resources point to different profiles")
        ),
        Status::Unmanaged => println!(
            "{}",
            style::warning("Active paths contain unmanaged files or links")
        ),
    }
    Ok(())
}
