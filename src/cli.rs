use clap::{Args, CommandFactory, Parser, Subcommand};

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

pub fn command_names() -> Vec<String> {
    let mut names = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect::<Vec<_>>();
    // Clap handles `help` as an implicit subcommand and does not expose it above.
    names.push("help".to_string());
    names
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
    /// Manage a target configured in ~/.cprof/extra-target.toml
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args)]
pub struct TargetArgs {
    #[command(subcommand)]
    pub command: TargetCommand,
}

#[derive(Parser)]
pub struct TargetCli {
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
        /// Copy an existing profile before editing
        #[arg(short, long, value_name = "PROFILE")]
        copy_from: Option<String>,
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
    /// Remove profiles by name or wildcard pattern
    Remove {
        #[arg(value_name = "NAME_OR_PATTERN")]
        names: Vec<String>,
    },
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
