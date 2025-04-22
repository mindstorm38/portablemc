//! Implementation of the 'auth' command.

use std::process::ExitCode;

use portablemc::msa::{Account, Auth, AuthError};
use uuid::Uuid;

use crate::output::LogLevel;
use crate::parse::AuthArgs;

use super::{Cli, log_msa_auth_error, log_msa_database_error};


pub fn auth(cli: &mut Cli, args: &AuthArgs) -> ExitCode {
    if let Some(forget_name) = &args.forget {
        auth_account_action(cli, forget_name, AccountAction::Forget)
    } else if let Some(refresh_name) = &args.refresh {
        auth_account_action(cli, refresh_name, AccountAction::Refresh)
    } else if args.list {
        auth_list(cli)
    } else {
        auth_login(cli, args.no_browser)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountAction {
    Forget,
    Refresh,
}

fn auth_account_action(cli: &mut Cli, name: &str, action: AccountAction) -> ExitCode {

    let res = 
    if let Ok(uuid) = Uuid::parse_str(&name) {
        match action {
            AccountAction::Forget => cli.msa_db.remove_from_uuid(uuid),
            AccountAction::Refresh => cli.msa_db.load_from_uuid(uuid),
        }
    } else {
        match action {
            AccountAction::Forget => cli.msa_db.remove_from_username(&name),
            AccountAction::Refresh => cli.msa_db.load_from_username(&name),
        }
    };

    let account = match res {
        Ok(Some(account)) => account,
        Ok(None) => {

            cli.out.log("auth_account_not_found")
                .arg(name)
                .warning(format_args!("No account found for: {name}"));

            return ExitCode::SUCCESS;

        }
        Err(error) => {
            log_msa_database_error(cli, &error);
            return ExitCode::FAILURE;
        }
    };

    match action {
        AccountAction::Forget => {

            cli.out.log("auth_account_forgot")
                .arg(account.uuid())
                .arg(account.username())
                .success(format_args!("Forgot account {} ({})", account.username(), account.uuid()));

            ExitCode::SUCCESS
            
        }
        AccountAction::Refresh => {

            if refresh_account(cli, account, false) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }

        }
    }

}

pub(crate) fn refresh_account(cli: &mut Cli, mut account: Account, silent: bool) -> bool {

    cli.out.log("auth_account_refresh_profile")
        .arg(account.uuid())
        .arg(account.username())
        .line(if silent { LogLevel::Info } else { LogLevel::Pending }, 
            format_args!("Refreshing account profile for {}", account.uuid()));

    let mut refreshed_token = false;

    match account.request_profile() {
        Ok(()) => {}
        Err(AuthError::OutdatedToken) => {
            
            cli.out.log("auth_account_refresh_token")
                .arg(account.uuid())
                .arg(account.username())
                .pending(format_args!("Refreshing account token for {}", account.uuid()));

            match account.request_refresh() {
                Ok(()) => {
                    refreshed_token = true;
                }
                Err(error) => {
                    log_msa_auth_error(cli, &error);
                    return false;
                }
            }

        }
        Err(error) => {
            log_msa_auth_error(cli, &error);
            return false;
        }
    };

    cli.out.log("auth_account_refreshed")
        .arg(account.uuid())
        .arg(account.username())
        .line(if silent && !refreshed_token { LogLevel::Info } else { LogLevel::Success }, 
            format_args!("Refreshed account as {} ({})", account.username(), account.uuid()));
    
    // Once the account is refreshed, store it!
    match cli.msa_db.store(account) {
        Ok(()) => true,
        Err(error) => {
            log_msa_database_error(cli, &error);
            false
        }
    }

}

fn auth_list(cli: &mut Cli) -> ExitCode {

    let iter = match cli.msa_db.load_iter() {
        Ok(iter) => iter,
        Err(error) => {
            log_msa_database_error(cli, &error);
            return ExitCode::FAILURE;
        }
    };

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

    ExitCode::SUCCESS

}

fn auth_login(cli: &mut Cli, no_browser: bool) -> ExitCode {

    let auth = Auth::new(&cli.msa_azure_app_id);

    cli.out.log("auth_request_device_code")
        .pending("Requesting authentication device code...");

    let code_flow = match auth.request_device_code() {
        Ok(ret) => ret,
        Err(error) => {
            log_msa_auth_error(cli, &error);
            return ExitCode::FAILURE;
        }
    };

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

    cli.out.log("auth_wait")
        .pending("Waiting for authentication to complete...");

    let account = match code_flow.wait() {
        Ok(account) => account,
        Err(error) => {
            log_msa_auth_error(cli, &error);
            return ExitCode::FAILURE;
        }
    };

    cli.out.log("auth_account_authenticated")
        .arg(account.uuid())
        .arg(account.username())
        .success(format_args!("Authenticated account as {} ({})", account.username(), account.uuid()));

    match cli.msa_db.store(account) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log_msa_database_error(cli, &error);
            ExitCode::FAILURE
        }
    }
    
}
