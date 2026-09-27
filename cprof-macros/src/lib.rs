//! Procedural macros shared by cprof commands and tests.

use proc_macro::TokenStream;

use syn::{ItemFn, parse_macro_input};

mod command;
mod windows_elevation;

/// Declare a synchronous `run` entry point and its elevation policy.
///
/// Use `#[command(no_retry)]` when the command has no safe elevation retry.
/// Otherwise, specify exactly one rollback-safe call with `retry`, the CLI
/// `args` to replay, and optional conditional `flags`.
///
/// Replay arguments are built only after a recoverable privilege error. The
/// selected call returns `Result<Option<T>>`: `Some(T)` on local success, or
/// `None` when an elevated child completes the command. A crate-visible
/// `COMMAND_MARKER` is emitted for command registration checks.
#[proc_macro_attribute]
pub fn command(args: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_macro_input!(args as command::Command);
    let function = parse_macro_input!(item as ItemFn);
    command::expand(options, function)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Keep a test enabled on Unix and ignored by default on Windows.
///
/// Place before `#[test]` on tests requiring symbolic-link privileges. Enable
/// Windows Developer Mode or use an elevated terminal, then run with
/// `--include-ignored`. This attribute does not launch UAC.
#[proc_macro_attribute]
pub fn windows_elevation(args: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    windows_elevation::expand(args.into(), function)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
