//! PortableMC CLI.

pub mod parse;
pub mod format;
pub mod output;
pub mod cmd;

use std::process::ExitCode;

use clap::Parser;

use parse::CliArgs;


/// Entry point.
fn main() -> ExitCode {
    cmd::main(CliArgs::parse())
}
