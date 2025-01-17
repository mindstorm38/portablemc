//! Implementation of the 'auth' command.

use std::process::ExitCode;

use portablemc::msa::{Auth, DatabaseError};
use uuid::Uuid;

use crate::parse::AuthArgs;

use super::{Cli, log_io_error};


pub fn auth(cli: &mut Cli, args: &AuthArgs) -> ExitCode {

    let res = 
    if let Some(forget_name) = &args.forget {
        auth_forget(cli, forget_name)
    } else if args.list {
        auth_list(cli)
    } else {
        auth_login(cli, args.no_browser)
    };

    if let Err(error) = res {
        log_database_error(&mut *cli, error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }

}

fn auth_forget(cli: &mut Cli, forget_name: &str) -> Result<(), DatabaseError> {

    let res = 
    if let Ok(forget_uuid) = Uuid::parse_str(&forget_name) {
        cli.msa_db.remove_from_uuid(forget_uuid)?
    } else {
        cli.msa_db.remove_from_username(&forget_name)?
    };

    if let Some(account) = res {
        cli.out.log("auth_account_forgot")
            .arg(account.uuid())
            .arg(account.username())
            .success(format_args!("Forgot account {} ({})", account.username(), account.uuid()));
    } else {
        cli.out.log("auth_account_not_found")
            .arg(forget_name)
            .error(format_args!("No account found for: {forget_name}"));
    }

    Ok(())

}

fn auth_list(cli: &mut Cli) -> Result<(), DatabaseError> {

    let iter = cli.msa_db.load_iter()?;

    // Now we construct the table...
    let mut table = cli.out.table(2);

    {
        let mut row = table.row();
        row.cell("username").format("Username");
        row.cell("uuid").format("UUID");
    }
    
    table.sep();

    for account in iter {
        let mut row = table.row();
        row.cell(account.username());
        row.cell(account.uuid());
    }

    Ok(())

}

fn auth_login(cli: &mut Cli, no_browser: bool) -> Result<(), DatabaseError> {

    let auth = Auth::new(crate::AZURE_APP_ID);

    cli.out.log("auth_request_device_code")
        .pending("Requesting authentication device code...");

    let code_flow = auth.request_device_code().unwrap();

    cli.out.log("auth_device_code")
        .arg(code_flow.verification_uri())
        .arg(code_flow.user_code())
        .success(code_flow.message());

    if cli.out.is_human() && !no_browser {
        if webbrowser::open(code_flow.verification_uri()).is_ok() {
            cli.out.log("auth_webbrowser_opened")
                .additional("Your web browser has been opened");
        }
    }

    cli.out.log("auth_waiting")
        .pending("Waiting for authentication to complete...");

    let account = code_flow.wait().unwrap();

    cli.out.log("auth_success")
        .arg(account.uuid())
        .arg(account.username())
        .success(format_args!("Successfully authenticated as {} ({})", account.username(), account.uuid()));

    cli.msa_db.store(account)
    
}

/// Log a database error.
pub fn log_database_error(cli: &mut Cli, error: DatabaseError) {
    match error {
        DatabaseError::Io(error) => log_io_error(cli, error, &format!("{}", cli.msa_db.file().display())),
        DatabaseError::Corrupted => {
            cli.out.log("error_msa_database_corrupted")
                .error("The authentication database is corrupted and cannot be recovered automatically")
                .additional(format_args!("At {}", cli.msa_db.file().display()));
        }
        DatabaseError::WriteFailed => {
            cli.out.log("error_msa_database_write_failed")
                .error("Unknown error while writing the authentication database")
                .additional(format_args!("At {}", cli.msa_db.file().display()));
        }
    }
}
