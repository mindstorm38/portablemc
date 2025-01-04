//! Implementation of the 'search' command.

use std::process::ExitCode;
use std::fs;

use chrono::{DateTime, Local, TimeDelta, Utc};

use portablemc::standard::VersionChannel;
use portablemc::{mojang, fabric, forge};

use crate::parse::{SearchArgs, SearchKind, SearchChannel, SearchLatestChannel};
use crate::format::{TimeDeltaFmt, DATE_FORMAT};

use super::{Cli, CommonHandler, log_mojang_error, log_io_error, log_forge_error};


pub fn main(cli: &mut Cli, args: &SearchArgs) -> ExitCode {
    
    match args.kind {
        SearchKind::Mojang => search_mojang(cli, args),
        SearchKind::Local => search_local(cli, args),
        SearchKind::Fabric => search_fabric(cli, args, fabric::Loader::Fabric),
        SearchKind::Quilt => search_fabric(cli, args, fabric::Loader::Quilt),
        SearchKind::LegacyFabric => search_fabric(cli, args, fabric::Loader::LegacyFabric),
        SearchKind::Babric => search_fabric(cli, args, fabric::Loader::Babric),
        SearchKind::Forge => search_forge(cli, args, forge::Loader::Forge),
        SearchKind::NeoForge => search_forge(cli, args, forge::Loader::NeoForge),
    }

}

// /// Common internal function to parse a search query.
// fn parse_query<P, V>(query: &[String], mut param: P, mut value: V)
// where
//     P: FnMut(&str, &str) -> bool,
//     V: FnMut(&str),
// {
//     for part in query {
//         if let Some((param, value)) = part.split_once(":") {
//             if !param(param, value) {
                
//             }
//         } else {
//             value(&part);
//         }
//     }
// }

fn search_mojang(cli: &mut Cli, args: &SearchArgs) -> ExitCode {
    
    use mojang::Manifest;

    // Initial requests...
    let mut handler = CommonHandler::new(&mut cli.out);
    let manifest = match Manifest::request(&mut handler) {
        Ok(manifest) => manifest,
        Err(e) => {
            log_mojang_error(&mut cli.out, e);
            return ExitCode::FAILURE;
        }
    };

    let today = Utc::now();

    // Now we construct the table...
    let mut table = cli.out.table(3);

    {
        let mut row = table.row();
        row.cell("name").format("Name");
        row.cell("channel").format("Channel");
        row.cell("release_date").format("Release date");
    }
    
    table.sep();

    // This is an exclusive argument.
    let only_name = args.latest.as_ref().map(|channel| {
        match channel {
            SearchLatestChannel::Release => manifest.latest_release_name(),
            SearchLatestChannel::Snapshot => manifest.latest_snapshot_name(),
        }
    });

    // Finally displaying version(s).
    for version in manifest.iter() {

        if let Some(only_name) = only_name {
            if version.name() != only_name {
                continue;
            }
        } else {

            if !args.filter.is_empty() {
                if !args.filter.iter().any(|s| version.name().contains(s)) {
                    continue;
                }
            }

            if !args.channel.is_empty() {
                let channel = match version.channel() {
                    VersionChannel::Release => SearchChannel::Release,
                    VersionChannel::Snapshot => SearchChannel::Snapshot,
                    VersionChannel::Beta => SearchChannel::Beta,
                    VersionChannel::Alpha => SearchChannel::Alpha,
                };
                if !args.channel.contains(&channel) {
                    continue;
                }
            }

        }
        
        let mut row = table.row();
        row.cell(version.name());
        
        let (channel_id, channel_fmt, is_latest) = match version.channel() {
            VersionChannel::Release => ("release", "Release", manifest.latest_release_name() == version.name()),
            VersionChannel::Snapshot => ("snapshot", "Snapshot", manifest.latest_snapshot_name() == version.name()),
            VersionChannel::Beta => ("beta", "Beta", false),
            VersionChannel::Alpha => ("alpha", "Alpha", false),
        };
        
        if is_latest {
            row.cell(format_args!("{channel_id}*")).format(format_args!("{channel_fmt}*"));
        } else {
            row.cell(format_args!("{channel_id}")).format(format_args!("{channel_fmt}"));
        }

        // Raw output is RFC3339 of FixedOffset time, format is local time.
        let mut cell = row.cell(&version.release_time().to_rfc3339());
        let local_release_date = version.release_time().with_timezone(&Local);
        let local_release_data_fmt: _ = version.release_time().format(DATE_FORMAT);
        let delta = today.signed_duration_since(&local_release_date);

        if is_latest || version.channel() == VersionChannel::Release || delta <= TimeDelta::weeks(4) {
            cell.format(format_args!("{} ({})", local_release_data_fmt, TimeDeltaFmt(delta)));
        } else {
            cell.format(format_args!("{}", local_release_data_fmt));
        }

    }

    ExitCode::SUCCESS

}

fn search_local(cli: &mut Cli, args: &SearchArgs) -> ExitCode {

    let reader = match fs::read_dir(&cli.versions_dir) {
        Ok(reader) => reader,
        Err(e) => {
            log_io_error(&mut cli.out, e, &format!("{}", cli.versions_dir.display()));
            return ExitCode::FAILURE;
        }
    };
    
    // Construct the table.
    let mut table = cli.out.table(2);

    {
        let mut row = table.row();
        row.cell("name").format("Name");
        row.cell("last_modified_date").format("Last modified date");
    }
    
    table.sep();

    for entry in reader {
        
        let Ok(entry) = entry else { continue };
        let Ok(entry_type) = entry.file_type() else { continue };
        if !entry_type.is_dir() { continue };

        let mut version_dir = entry.path();
        let Some(version_id) = version_dir.file_name().unwrap().to_str() else { continue };
        let version_id = version_id.to_string();

        version_dir.push(&version_id);
        version_dir.as_mut_os_string().push(".json");

        let Ok(version_metadata) = version_dir.metadata() else { continue };
        let Ok(version_last_modified) = version_metadata.modified() else { continue };
        let version_last_modified = DateTime::<Local>::from(version_last_modified);

        if !args.filter.is_empty() {
            if !args.filter.iter().any(|s| version_id.contains(s)) {
                continue;
            }
        }
        
        // We use the local timezone for both raw and format cells.
        let mut row = table.row();
        row.cell(&version_id);
        row.cell(&version_last_modified.to_rfc3339())
            .format(version_last_modified.format(DATE_FORMAT));

    }

    ExitCode::SUCCESS

}

fn search_fabric(cli: &mut Cli, args: &SearchArgs, loader: fabric::Loader) -> ExitCode {

    let today = Utc::now();

    // Now we construct the table...
    let mut table = cli.out.table(2);

    {
        let mut row = table.row();
        row.cell("version").format("Version");
        row.cell("channel").format("Channel");
    }
    
    table.sep();

    ExitCode::SUCCESS

}

fn search_forge(cli: &mut Cli, args: &SearchArgs, loader: forge::Loader) -> ExitCode {

    use forge::Repo;

    // Start by requesting the repository!
    let  repo = match Repo::request(loader) {
        Ok(repo) => repo,
        Err(e) => {
            log_forge_error(&mut cli.out, e, "neoforge", "NeoForge");
            return ExitCode::FAILURE;
        }
    };

    // Now we construct the table...
    let mut table = cli.out.table(3);

    {
        let mut row = table.row();
        row.cell("version").format("Version");
        row.cell("game_version").format("Game version");
        row.cell("channel").format("Channel");
    }
    
    table.sep();

    for version in repo.iter() {
        
        if !args.filter.is_empty() {
            if !args.filter.iter().any(|s| version.name().contains(s)) {
                continue;
            }
        }

        if !args.game_version.is_empty() {
            if !args.game_version.iter().any(|s| version.game_version() == s) {
                continue;
            }
        }

        if !args.channel.is_empty() {
            let stable = version.is_stable();
            if !args.channel.iter().any(|c| match c {
                SearchChannel::Stable => stable,
                SearchChannel::Unstable => !stable,
                _ => false,
            }) {
                continue;
            }
        }

        let mut row = table.row();
        row.cell(version.name());
        row.cell(version.game_version());
        row.cell(if version.is_stable() { "stable" } else { "unstable" })
            .format(if version.is_stable() { "Stable" } else { "Unstable" });

    }

    ExitCode::SUCCESS

}
