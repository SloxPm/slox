use clap::Parser;
use slox_cli::Args;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = Args::parse();

    match slox_core::run(args.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            slox_core::report_error(&error);
            ExitCode::FAILURE
        }
    }
}
