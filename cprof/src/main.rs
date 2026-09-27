use std::env;
use std::ffi::OsString;
use std::iter::once;
use std::process::ExitCode;

use clap::Parser;

use cprof::cli::{self, Cli};
use cprof::commands;
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
            let exit_code = elevate::error_exit_code(&error);
            if !elevate::is_elevated_child() {
                style::error(error);
            }
            ExitCode::from(exit_code)
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

    commands::dispatch_root(Cli::try_parse_from(once(OsString::from("cprof")).chain(args))?.command)
}
