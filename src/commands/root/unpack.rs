use crate::commands::target;
use crate::error::Result;
use crate::style;
use crate::targets::TargetRepository;

pub fn run(targets: &TargetRepository, path: Option<String>, force: bool) -> Result<()> {
    let packages = target::unpack::read_package(targets, path)?;
    let (target_configs, configs_changed) =
        target::unpack::merge_target_configs(targets, &packages, force)?;
    let mut plans = Vec::with_capacity(packages.len());
    for package in packages {
        let target = target::unpack::resolve_target(targets, &package, &target_configs)?;
        let profiles = target::unpack::remap_profiles(&target, package.profiles)?;
        plans.push(target::unpack::prepare_unpack(target, profiles, force)?);
    }

    target::unpack::commit_unpack(&plans, &target_configs, configs_changed)?;
    let mut targets = 0;
    let mut unpacked = 0;
    let mut skipped = 0;
    for plan in &plans {
        let (written, ignored) = plan.report();
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
