use clap::Parser;
use plox::cli::Args;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = Args::parse();

    match plox::run(args.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            plox::report_error(&error);
            ExitCode::FAILURE
        }
    }
}
