use clap::Parser;
use cprof::{cli, commands, style, targets};

use cli::{Cli, RootCommand, TargetCommand};

fn main() {
    let result = match Cli::parse().command {
        RootCommand::Claude(args) => run_target_command("claude", args.command),
        RootCommand::Codex(args) => run_target_command("codex", args.command),
        RootCommand::Pack(args) => commands::pack::run_all(args.save),
        RootCommand::Unpack(args) => commands::unpack::run_all(args.path, args.force),
    };

    if let Err(error) = result {
        eprintln!("{} {}", style::error_label(), error);
        std::process::exit(1);
    }
}

fn run_target_command(target_id: &str, command: TargetCommand) -> cprof::error::Result<()> {
    let target = targets::get(target_id)?;
    match command {
        TargetCommand::Dir => commands::dir::run(target),
        TargetCommand::List => commands::list::run(target),
        TargetCommand::Which => commands::which::run(target),
        TargetCommand::Num => commands::num::run(target),
        TargetCommand::Create { name, editor } => commands::create::run(target, name, editor),
        TargetCommand::Edit {
            name,
            filename,
            editor,
        } => commands::edit::run(target, name, filename, editor),
        TargetCommand::Remove { names } => commands::remove::run(target, names),
        TargetCommand::Switch { name, force } => commands::switch::run(target, name, force),
        TargetCommand::Clean { force } => commands::clean::run(target, force),
        TargetCommand::Where { name, filename } => commands::where_::run(target, name, filename),
        TargetCommand::Pack(args) => commands::pack::run(target, args.save),
        TargetCommand::Unpack(args) => commands::unpack::run(target, args.path, args.force),
    }
}
