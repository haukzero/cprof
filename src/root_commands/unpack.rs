use crate::commands;
use crate::error::Result;
use crate::style;

pub fn run(path: Option<String>, force: bool) -> Result<()> {
    let packages = commands::unpack::read_package(path)?;
    let target_configs = commands::unpack::merge_target_configs(&packages, force)?;
    let mut targets = 0;
    let mut unpacked = 0;
    let mut skipped = 0;
    for package in packages {
        let target = commands::unpack::resolve_target(&package, &target_configs)?;
        println!(
            "{}",
            style::heading(&format!("Unpacking target '{}':", target.id))
        );
        let profiles = commands::unpack::remap_profiles(target, package.profiles)?;
        let (written, ignored) = commands::unpack::unpack_target(target, profiles, force)?;
        targets += 1;
        unpacked += written;
        skipped += ignored;
    }
    println!(
        "\n{} {unpacked} unpacked, {skipped} skipped across {targets} target(s)",
        style::success("Done!")
    );
    Ok(())
}
