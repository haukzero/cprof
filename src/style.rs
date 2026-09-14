use colored::ColoredString;
use colored::Colorize;

/// Active profile indicator
pub fn active_tag() -> ColoredString {
    "(active)".green()
}

/// Success message
pub fn success(msg: &str) -> ColoredString {
    msg.green().bold()
}

/// Warning message
pub fn warning(msg: &str) -> ColoredString {
    msg.yellow()
}

/// Error label
pub fn error_label() -> ColoredString {
    "error:".red().bold()
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
