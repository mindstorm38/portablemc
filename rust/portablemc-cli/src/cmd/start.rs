//! Implementation of the 'start' command.

use std::process::ExitCode;

use portablemc::mojang::Handler as _;
use portablemc::{mojang, standard};

use crate::parse::{StartArgs, StartResolution, StartVersion};

use super::{Cli, CommonHandler, log_mojang_error};


pub fn main(cli: &mut Cli, args: &StartArgs) -> ExitCode {

    let game;

    match &args.version {
        StartVersion::Mojang { 
            root,
        } => {
            
            let mut inst = mojang::Installer::new(cli.main_dir.clone());
            apply_mojang_args(&mut inst, &cli, args);
            inst.root(root.clone());

            let mut handler = CommonHandler::new(&mut cli.out);
            game = match inst.install(handler.as_mojang_dyn()) {
                Ok(game) => game,
                Err(e) => {
                    log_mojang_error(&mut cli.out, e);
                    return ExitCode::FAILURE;
                }
            };

        }
        StartVersion::Loader {
            root, 
            loader, 
            kind,
        } => {
            let _ = (root, loader, kind);
            todo!("start loader");
        }
    }

    if args.dry {
        return ExitCode::SUCCESS;
    }
    
    let _ = game;

    todo!()

}


// Internal function to apply args to the standard installer.
pub fn apply_standard_args<'a>(
    installer: &'a mut standard::Installer, 
    cli: &Cli, 
) -> &'a mut standard::Installer {
    installer.versions_dir(cli.versions_dir.clone());
    installer.libraries_dir(cli.libraries_dir.clone());
    installer.assets_dir(cli.assets_dir.clone());
    installer.jvm_dir(cli.jvm_dir.clone());
    installer.bin_dir(cli.bin_dir.clone());
    installer.work_dir(cli.work_dir.clone());
    installer
}

// Internal function to apply args to the mojang installer.
pub fn apply_mojang_args<'a>(
    installer: &'a mut mojang::Installer,
    cli: &Cli, 
    args: &StartArgs,
) -> &'a mut mojang::Installer {

    installer.with_standard(|i| apply_standard_args(i, cli));
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
