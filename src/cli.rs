use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cprof", version, about = "Claude Code profile manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show the profiles directory path
    Dir,

    /// List all profiles (active one highlighted)
    List,

    /// Show the currently active profile name
    Which,

    /// Show the total number of profiles
    Num,

    /// Create a new profile
    Create {
        /// Profile name (prompts if omitted)
        name: Option<String>,
        /// Editor to use (overrides default)
        #[arg(long)]
        editor: Option<String>,
    },

    /// Edit an existing profile
    Edit {
        /// Profile name (prompts if omitted)
        name: Option<String>,
        /// Editor to use (overrides default)
        #[arg(long)]
        editor: Option<String>,
    },

    /// Remove one or more profiles
    Remove {
        /// Profile names (space-separated, prompts if omitted)
        names: Vec<String>,
    },

    /// Switch to a profile
    Switch {
        /// Profile name (prompts if omitted)
        name: Option<String>,
    },

    /// Remove all profiles and the symlink
    Clean {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Actual filepath of a profile
    Where {
        /// Profile name (prompts if omitted)
        name: Option<String>,
    },

    /// Pack profiles into a portable package
    Pack {
        /// Output file path (default: ./cprof.pkg)
        #[arg(long)]
        save: Option<String>,
    },

    /// Unpack a package file
    Unpack {
        /// Package file path (default: ./cprof.pkg)
        #[arg(long)]
        path: Option<String>,
    },
}
