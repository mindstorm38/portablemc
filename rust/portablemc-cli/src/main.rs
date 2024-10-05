//! PortableMC CLI.

pub mod parse;
pub mod output;

use clap::Parser;

use output::human::HumanHandler;
use output::machine::MachineHandler;
use portablemc::{download, standard, mojang};
use portablemc::msa;

use parse::{CliArgs, CliCmd, CliOutput, LoginArgs, LogoutArgs, SearchArgs, ShowArgs, StartArgs, StartResolution, StartVersion};


fn main() {
    let args = CliArgs::parse();
    println!("{args:?}");
    cmd_cli(&args);
}

fn cmd_cli(args: &CliArgs) {
    match &args.cmd {
        CliCmd::Search(search_args) => cmd_search(args, search_args),
        CliCmd::Start(start_args) => cmd_start(args, start_args),
        CliCmd::Login(login_args) => cmd_login(args, login_args),
        CliCmd::Logout(logout_args) => cmd_logout(args, logout_args),
        CliCmd::Show(show_args) => cmd_show(args, show_args),
    }
}

fn cmd_search(cli_args: &CliArgs, args: &SearchArgs) {
    
}

fn cmd_start(cli_args: &CliArgs, args: &StartArgs) {
    
    // Internal function to apply args to the standard installer.
    fn apply_standard_args<'a>(
        installer: &'a mut standard::Installer, 
        cli_args: &CliArgs, 
        _args: &StartArgs,
    ) -> &'a mut standard::Installer {
        
        if let Some(dir) = &cli_args.main_dir {
            installer.main_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.versions_dir {
            installer.versions_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.libraries_dir {
            installer.libraries_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.assets_dir {
            installer.assets_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.jvm_dir {
            installer.jvm_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.bin_dir {
            installer.bin_dir(dir.clone());
        }
        if let Some(dir) = &cli_args.work_dir {
            installer.work_dir(dir.clone());
        }

        installer
        
    }

    // Internal function to apply args to the mojang installer.
    fn apply_mojang_args<'a>(
        installer: &'a mut mojang::Installer,
        cli_args: &CliArgs, 
        args: &StartArgs,
    ) -> &'a mut mojang::Installer {

        installer.with_standard(|i| apply_standard_args(i, cli_args, args));
        installer.disable_multiplayer(args.disable_multiplayer);
        installer.disable_chat(args.disable_chat);
        installer.demo(args.demo);

        if let Some(StartResolution { width, height }) = args.resolution {
            installer.resolution(width, height);
        }

        if let Some(lwjgl) = &args.lwjgl {
            installer.fix_lwjgl(lwjgl.to_string());
        }

        for exclude_id in &args.exclude_fetch {
            if exclude_id == "*" {
                installer.fetch(false);
            } else {
                installer.fetch_exclude(exclude_id.clone());
            }
        }

        match (&args.username, &args.uuid) {
            (Some(username), None) => 
                installer.auth_offline_username_authlib(username.clone()),
            (None, Some(uuid)) =>
                installer.auth_offline_uuid(*uuid),
            (Some(username), Some(uuid)) =>
                installer.auth_offline(*uuid, username.clone()),
            (None, None) => installer, // nothing
        };

        if let Some(server) = &args.server {
            installer.quick_play(mojang::QuickPlay::Multiplayer { 
                host: server.clone(), 
                port: args.server_port,
            });
        }
        
        installer

    }

    let mut handler = match cli_args.output {
        None | 
        Some(CliOutput::HumanColor) => Handler::Human(HumanHandler::new(true)),
        Some(CliOutput::Human) => Handler::Human(HumanHandler::new(false)),
        Some(CliOutput::Machine) => Handler::Machine(MachineHandler::new()),
    };

    let game;

    match &args.version {
        StartVersion::Mojang { 
            root,
        } => {
            
            let mut inst = mojang::Installer::new();
            apply_mojang_args(&mut inst, cli_args, args);
            inst.root(root.clone());
            game = inst.install(&mut handler).unwrap();

        }
        StartVersion::Loader {
            root, 
            loader, 
            kind,
        } => {
            todo!("start loader");
        }
    }

    if args.dry {
        return;
    }
    
    let _ = game;

    todo!()

}

fn cmd_login(cli_args: &CliArgs, args: &LoginArgs) {

}

fn cmd_logout(cli_args: &CliArgs, args: &LogoutArgs) {

}

fn cmd_show(cli_args: &CliArgs, args: &ShowArgs) {

}






#[allow(unused)]
fn test_auth() -> msa::Result<()> {

    let auth = msa::Auth::new("708e91b5-99f8-4a1d-80ec-e746cbb24771".to_string());
    let device_code_auth = auth.request_device_code()?;
    println!("{}", device_code_auth.message());

    let account = device_code_auth.wait()?;
    println!("account: {account:#?}");

    Ok(())

}

/// Generic installation handler.
#[derive(Debug)]
enum Handler {
    Human(HumanHandler),
    Machine(MachineHandler),
}

impl download::Handler for Handler {
    fn handle_download_progress(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) {
        match self {
            Handler::Human(handler) => handler.handle_download_progress(count, total_count, size, total_size),
            Handler::Machine(handler) => handler.handle_download_progress(count, total_count, size, total_size),
        }
    }
}

impl standard::Handler for Handler {
    fn handle_standard_event(&mut self, event: standard::Event) {
        match self {
            Handler::Human(handler) => handler.handle_standard_event(event),
            Handler::Machine(handler) => handler.handle_standard_event(event),
        }
    }
}

impl mojang::Handler for Handler {
    fn handle_mojang_event(&mut self, event: mojang::Event) {
        match self {
            Handler::Human(handler) => handler.handle_mojang_event(event),
            Handler::Machine(handler) => handler.handle_mojang_event(event),
        }
    }
}
