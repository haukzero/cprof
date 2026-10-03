use std::env;

/// An environment setting with a fixed value type.
pub trait EnvVar {
    type Value;

    /// Environment variable names in descending priority order.
    fn names(&self) -> &[&str];

    /// Raw value used when no nonblank environment value is available.
    fn default(&self) -> &str;

    /// Resolve fallbacks, preserving the selected value's whitespace and quoting.
    fn read(&self) -> String {
        self.names()
            .iter()
            .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()))
            .unwrap_or_else(|| self.default().to_string())
    }

    fn get(self) -> Self::Value;
}

/// CPROF_ASCII: use ASCII tree connectors when set to 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ascii;

impl EnvVar for Ascii {
    type Value = bool;

    fn names(&self) -> &[&str] {
        &["CPROF_ASCII"]
    }

    fn default(&self) -> &str {
        "0"
    }

    fn get(self) -> Self::Value {
        self.read() == "1"
    }
}

/// CPROF_EDITOR: editor command, falling back to VISUAL and EDITOR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Editor;

impl EnvVar for Editor {
    type Value = String;

    fn names(&self) -> &[&str] {
        &["CPROF_EDITOR", "VISUAL", "EDITOR"]
    }

    fn default(&self) -> &str {
        if cfg!(windows) { "notepad" } else { "vi" }
    }

    fn get(self) -> Self::Value {
        self.read()
    }
}
