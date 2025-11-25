//! PortableMC CLI.

pub mod parse;
pub mod format;
pub mod output;
pub mod cmd;

use std::process::ExitCode;

use clap::Parser;

use parse::CliArgs;


const AZURE_APP_ID: &str = "708e91b5-99f8-4a1d-80ec-e746cbb24771";


/// Entry point.
fn main() -> ExitCode {
    cmd::main(&CliArgs::parse())
}
