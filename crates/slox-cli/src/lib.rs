use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    Activate {
        #[command(subcommand)]
        command: ActivateCommand,
    },
    Pkg {
        #[command(subcommand)]
        command: PkgCommand,
    },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ActivateCommand {
    Sh,
    Bash,
    Zsh,
    Nu,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum EnvCommand {
    Add { path: String },
    Remove { path: String },
    Set { path: String },
    Fetch { path: String },
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum PkgCommand {
    Add { path: String },
    Remove { path: String },
}
