mod cli;
mod commands;
mod config;
mod editor;
mod error;
mod package;
mod profile;
mod prompt;
mod style;

use clap::Parser;

use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Dir => commands::dir::run(),
        Commands::List => commands::list::run(),
        Commands::Which => commands::which::run(),
        Commands::Num => commands::num::run(),
        Commands::Create { name, editor } => commands::create::run(name, editor),
        Commands::Edit { name, editor } => commands::edit::run(name, editor),
        Commands::Remove { names } => commands::remove::run(names),
        Commands::Switch { name } => commands::switch::run(name),
        Commands::Clean { force } => commands::clean::run(force),
        Commands::Pack { save } => commands::pack::run(save),
        Commands::Unpack { path } => commands::unpack::run(path),
    };

    if let Err(e) = result {
        eprintln!("{} {}", style::error_label(), e);
        std::process::exit(1);
    }
}
