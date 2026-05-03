use clap::Parser;
use slox::cli::Args;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = Args::parse();

    match slox::run(args.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            slox::report_error(&error);
            ExitCode::FAILURE
        }
    }
}
