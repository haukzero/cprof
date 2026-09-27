use std::iter::once;

use clap::Parser;
use clap::error::ErrorKind;

use crate::cli::{RootCommand, TargetCli, TargetCommand};
use crate::error::Result;
use crate::targets::TargetRepository;

pub mod root;
pub(crate) mod support;
pub mod target;

#[cprof_macros::command_dispatch]
pub fn dispatch_root(command: RootCommand) -> Result<()> {
    match command {
        RootCommand::Claude(args) => run_target_command("claude", args.command)?,
        RootCommand::Codex(args) => run_target_command("codex", args.command)?,
        RootCommand::Pack(args) => {
            let targets = TargetRepository::load()?;
            root::pack::run(&targets, args.options.save, args.select)?;
        }
        RootCommand::Unpack(args) => {
            let targets = TargetRepository::load()?;
            root::unpack::run(&targets, args.path, args.force, args.dry_run, args.mirror)?;
        }
        RootCommand::Clean(args) => {
            let targets = TargetRepository::load()?;
            root::clean::run(&targets, args.force, args.extra_toml)?;
        }
        RootCommand::EditExtra(args) => root::edit_extra::run(args.editor)?,
        RootCommand::Targets(args) => {
            let targets = TargetRepository::load()?;
            root::targets::run(&targets, args.json)?;
        }
        RootCommand::External(args) => run_external_command(args)?,
    }
    Ok(())
}

#[cprof_macros::command_dispatch]
fn dispatch_target(
    targets: &TargetRepository,
    target_id: &str,
    command: TargetCommand,
) -> Result<()> {
    let target = targets.get(target_id)?;
    match command {
        TargetCommand::Dir => target::dir::run(&target),
        TargetCommand::List => target::list::run(&target),
        TargetCommand::Which => target::which::run(&target),
        TargetCommand::Num => target::num::run(&target),
        TargetCommand::Create {
            name,
            copy_from,
            editor,
        } => target::create::run(&target, name, copy_from, editor),
        TargetCommand::Adopt { name } => target::adopt::run(&target, name),
        TargetCommand::Edit {
            name,
            filename,
            editor,
        } => target::edit::run(&target, name, filename, editor),
        TargetCommand::Remove { names, force } => target::remove::run(&target, names, force),
        TargetCommand::Rename { old_name, new_name } => {
            target::rename::run(&target, old_name, new_name)
        }
        TargetCommand::Switch { name, force } => target::switch::run(&target, name, force),
        TargetCommand::Clean { force } => target::clean::run(&target, force),
        TargetCommand::Where { name, filename } => target::where_::run(&target, name, filename),
        TargetCommand::Pack(args) => target::pack::run(targets, &target, args.save),
        TargetCommand::Unpack(args) => target::unpack::run(
            targets,
            &target,
            args.path,
            args.force,
            args.dry_run,
            args.mirror,
        ),
    }
}

fn run_target_command(target_id: &str, command: TargetCommand) -> Result<()> {
    let targets = TargetRepository::load()?;
    dispatch_target(&targets, target_id, command)
}

fn run_external_command(args: Vec<String>) -> Result<()> {
    let target_id = args
        .first()
        .cloned()
        .ok_or_else(|| clap::Error::raw(ErrorKind::MissingSubcommand, "Missing target command"))?;
    let targets = TargetRepository::load()?;
    targets.get(&target_id)?;
    let command_name = format!("cprof {target_id}");
    let command = TargetCli::try_parse_from(once(command_name).chain(args.into_iter().skip(1)))?;
    dispatch_target(&targets, &target_id, command.command)
}
