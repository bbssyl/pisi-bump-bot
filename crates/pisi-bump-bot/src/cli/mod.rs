mod args;
mod merge_command;
mod prepare_command;
mod render_command;
mod report_command;
mod runtime;

use std::ffi::OsString;

use clap::Parser;

pub use runtime::Runtime;

use args::{Cli, Command};

pub fn run<I, T>(args: I, runtime: Runtime) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let parsed = match Cli::try_parse_from(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            let _ = error.print();
            return error.exit_code();
        }
    };
    match parsed.command {
        Command::Report(report_args) => report_command::run(report_args, runtime),
        Command::Prepare(prepare_args) => prepare_command::run(prepare_args, runtime.download),
        Command::MergeResults(merge_args) => merge_command::run(merge_args),
        Command::Render(render_args) => render_command::run(render_args),
    }
}
