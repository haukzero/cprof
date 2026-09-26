use std::env;
use std::ffi::OsString;
use std::iter::once;
use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;

use cprof::cli::{self, Cli, RootCommand, TargetCli, TargetCommand};
use cprof::commands::{root, target};
use cprof::elevate;
use cprof::error::{AppError, Result};
use cprof::targets::TargetRepository;
use cprof::ui::style;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Cli(error)) => {
            let exit_code = error.exit_code();
            if let Err(print_error) = error.print() {
                style::error(print_error);
                return ExitCode::FAILURE;
            }
            ExitCode::from(u8::try_from(exit_code).unwrap_or(1))
        }
        Err(error) => {
            if !elevate::is_elevated_child() {
                style::error(error);
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args = elevate::prepare_args(env::args_os().skip(1).collect())?;
    if cli::is_root_help_request(&args) {
        let targets = TargetRepository::load()?;
        let command_names = cli::command_names();
        let extra_target_ids = targets
            .all()
            .iter()
            .filter(|target| !command_names.iter().any(|name| name == &target.id))
            .map(|target| target.id.clone());
        cli::command_with_extra_targets(extra_target_ids).print_help()?;
        println!();
        return Ok(());
    }

    let cli = Cli::try_parse_from(once(OsString::from("cprof")).chain(args))?;
    match cli.command {
        RootCommand::Claude(args) => run_target_command("claude", args.command)?,
        RootCommand::Codex(args) => run_target_command("codex", args.command)?,
        RootCommand::Pack(args) => {
            let targets = TargetRepository::load()?;
            root::pack::run(&targets, args.options.save, args.select)?;
        }
        RootCommand::Unpack(args) => {
            let targets = TargetRepository::load()?;
            root::unpack::run(&targets, args.path, args.force, args.dry_run)?;
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
        RootCommand::External(args) => {
            let target_id = args.first().cloned().ok_or_else(|| {
                clap::Error::raw(ErrorKind::MissingSubcommand, "Missing target command")
            })?;
            let targets = TargetRepository::load()?;
            targets.get(&target_id)?;
            let target_command = parse_target_command(&target_id, args.into_iter().skip(1))?;
            run_target_command_with_repository(&targets, &target_id, target_command)?;
        }
    }
    Ok(())
}

fn parse_target_command(
    target_id: &str,
    args: impl IntoIterator<Item = String>,
) -> Result<TargetCommand> {
    let command_name = format!("cprof {target_id}");
    match TargetCli::try_parse_from(once(command_name).chain(args)) {
        Ok(cli) => Ok(cli.command),
        Err(error) => Err(error.into()),
    }
}

fn run_target_command(target_id: &str, command: TargetCommand) -> Result<()> {
    let targets = TargetRepository::load()?;
    run_target_command_with_repository(&targets, target_id, command)?;
    Ok(())
}

fn run_target_command_with_repository(
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
        TargetCommand::Unpack(args) => {
            target::unpack::run(targets, &target, args.path, args.force, args.dry_run)
        }
    }
}
