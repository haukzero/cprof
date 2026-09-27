use std::fmt::Display;

use colored::ColoredString;
use colored::Colorize;

/// Active profile indicator
pub fn active_tag() -> ColoredString {
    "(active)".green()
}

/// Print a success message to standard output.
pub fn success(message: impl Display) {
    println!("{} {message}", "Done!".green().bold());
}

/// Print a warning to standard error.
pub fn warning(message: impl Display) {
    eprintln!("{} {message}", "warning:".yellow().bold());
}

/// Print an error to standard error.
pub fn error(message: impl Display) {
    eprintln!("{} {message}", "error:".red().bold());
}

/// Profile name when active
pub fn active_name(name: &str) -> ColoredString {
    name.green().bold()
}

/// Info/label text
pub fn label(msg: &str) -> ColoredString {
    msg.cyan()
}

/// Emphasized heading text
pub fn heading(msg: &str) -> ColoredString {
    msg.bold()
}

/// Marker for a configuration item that will be added.
pub fn added_marker() -> ColoredString {
    "+".green()
}

/// Marker for a configuration item that will be modified.
pub fn modified_marker() -> ColoredString {
    "M".yellow()
}

/// Marker for a configuration item that will be deleted.
pub fn deleted_marker() -> ColoredString {
    "-".red()
}
