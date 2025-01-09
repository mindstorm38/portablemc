
use std::process::ExitCode;

use portablemc::msa::Auth;

use crate::parse::LoginArgs;

use super::Cli;


pub fn main(cli: &mut Cli, args: &LoginArgs) -> ExitCode {

    let auth = Auth::new(crate::AZURE_APP_ID);
    let code_flow = auth.request_device_code(true).unwrap();

    cli.out.log("auth")
        .arg(code_flow.verification_uri())
        .arg(code_flow.user_code())
        .success(code_flow.message());

    let account = code_flow.wait().unwrap();

    cli.out.log("auth_success")
        .success(format_args!("{account:?}"));

    ExitCode::SUCCESS

}
