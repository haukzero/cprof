use clap::{Parser, error::ErrorKind};

use cprof::cli::{self, Cli, RootCommand, TargetCli, TargetCommand};
use cprof::commands::{root, target};
use cprof::{style, targets};

fn main() {
    let result = run();

    if let Err(error) = result {
        if cprof::elevate::is_elevated_child() {
            std::process::exit(1);
        }
        eprintln!("{} {}", style::error_label(), error);
        std::process::exit(1);
    }
}

fn run() -> cprof::error::Result<()> {
    let command = match Cli::try_parse() {
        Ok(cli) => cli.command,
        Err(error) if cli::is_root_help_error(&error) => {
            let command_names = cli::command_names();
            let extra_target_ids = targets::all()?.iter().filter_map(|target| {
                (!command_names.iter().any(|name| name == target.id)).then_some(target.id)
            });
            cli::command_with_extra_targets(extra_target_ids).print_help()?;
            return Ok(());
        }
        Err(error) => error.exit(),
    };
    match command {
        RootCommand::Claude(args) => run_target_command("claude", args.command),
        RootCommand::Codex(args) => run_target_command("codex", args.command),
        RootCommand::Pack(args) => root::pack::run(args.save),
        RootCommand::Unpack(args) => root::unpack::run(args.path, args.force),
        RootCommand::Clean(args) => root::clean::run(args.force, args.extra_toml),
        RootCommand::EditExtra(args) => root::edit_extra::run(args.editor),
        RootCommand::Targets => root::targets::run(),
        RootCommand::External(args) => {
            let target_id = args.first().cloned().ok_or_else(|| {
                cprof::error::AppError::Other("Missing target command".to_string())
            })?;
            targets::get(&target_id)?;
            let target_command = parse_target_command(&target_id, args.into_iter().skip(1))?;
            run_target_command(&target_id, target_command)
        }
    }
}

fn parse_target_command(
    target_id: &str,
    args: impl IntoIterator<Item = String>,
) -> cprof::error::Result<TargetCommand> {
    let command_name = format!("cprof {target_id}");
    match TargetCli::try_parse_from(std::iter::once(command_name).chain(args)) {
        Ok(cli) => Ok(cli.command),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            std::process::exit(0);
        }
        Err(error) => error.exit(),
    }
}

fn run_target_command(target_id: &str, command: TargetCommand) -> cprof::error::Result<()> {
    let target = targets::get(target_id)?;
    match command {
        TargetCommand::Dir => target::dir::run(target),
        TargetCommand::List => target::list::run(target),
        TargetCommand::Which => target::which::run(target),
        TargetCommand::Num => target::num::run(target),
        TargetCommand::Create {
            name,
            copy_from,
            editor,
        } => target::create::run(target, name, copy_from, editor),
        TargetCommand::Edit {
            name,
            filename,
            editor,
        } => target::edit::run(target, name, filename, editor),
        TargetCommand::Remove { names } => target::remove::run(target, names),
        TargetCommand::Switch { name, force } => target::switch::run(target, name, force),
        TargetCommand::Clean { force } => target::clean::run(target, force),
        TargetCommand::Where { name, filename } => target::where_::run(target, name, filename),
        TargetCommand::Pack(args) => target::pack::run(target, args.save),
        TargetCommand::Unpack(args) => target::unpack::run(target, args.path, args.force),
    }
}
