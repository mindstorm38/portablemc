//! PortableMC CLI.

pub mod parse;
pub mod format;
pub mod output;
pub mod cmd;
// pub mod auth;

use std::process::ExitCode;

use clap::Parser;

use parse::CliArgs;


/// Entry point.
fn main() -> ExitCode {
    cmd::main(CliArgs::parse())
}
