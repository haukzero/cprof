use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cprof",
    version,
    about = "Configuration profile manager",
    arg_required_else_help = true,
    subcommand_required = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: RootCommand,
}

#[derive(Subcommand)]
pub enum RootCommand {
    /// Manage Claude Code profiles
    #[command(name = "claude")]
    Claude(TargetArgs),
    /// Manage Codex profiles
    #[command(name = "codex")]
    Codex(TargetArgs),
    /// Pack profiles from every registered target
    Pack(PackArgs),
    /// Unpack profiles for every target in a package
    Unpack(UnpackArgs),
}

#[derive(Args)]
pub struct TargetArgs {
    #[command(subcommand)]
    pub command: TargetCommand,
}

#[derive(Args)]
pub struct PackArgs {
    /// Output file path (default: ./cprof.pkg)
    #[arg(long, value_name = "PATH")]
    pub save: Option<String>,
}

#[derive(Args)]
pub struct UnpackArgs {
    /// Package file path (default: ./cprof.pkg)
    #[arg(long, value_name = "PATH")]
    pub path: Option<String>,
    /// Force overwrite all existing profiles
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Subcommand)]
pub enum TargetCommand {
    /// Show the profiles directory path
    Dir,
    /// List all profiles (active one highlighted)
    List,
    /// Show the currently active profile name or status
    Which,
    /// Show the total number of profiles
    Num,
    /// Create a profile
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
        /// Edit only this logical resource, for example "auth"
        #[arg(long)]
        filename: Option<String>,
        /// Editor to use (overrides default)
        #[arg(long)]
        editor: Option<String>,
    },
    /// Remove one or more profiles
    Remove { names: Vec<String> },
    /// Switch to a profile
    Switch {
        /// Profile name (prompts if omitted)
        name: Option<String>,
        /// Replace unmanaged files or external symlinks at target paths
        #[arg(short, long)]
        force: bool,
    },
    /// Remove all profiles and managed symlinks for this target
    Clean {
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
    /// Show profile resource file paths
    Where {
        /// Profile name (prompts if omitted)
        name: Option<String>,
        /// Show only this logical resource, for example "auth"
        #[arg(long)]
        filename: Option<String>,
    },
    /// Pack profiles into a portable package
    Pack(PackArgs),
    /// Unpack a portable package
    Unpack(UnpackArgs),
}
