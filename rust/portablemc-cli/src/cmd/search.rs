//! Implementation of the 'search' command.

use std::process::ExitCode;
use std::fs;

use chrono::{DateTime, Local, TimeDelta, Utc};

use portablemc::standard::VersionChannel;
use portablemc::download::Handler;
use portablemc::mojang::Manifest;
use portablemc::fabric;

use crate::parse::{SearchArgs, SearchKind, SearchChannel, SearchLatestChannel};
use crate::format::{TimeDeltaFmt, DATE_FORMAT};

use super::{Cli, CommonHandler, log_mojang_error, log_io_error};


pub fn main(cli: &mut Cli, args: &SearchArgs) -> ExitCode {
    
    match args.kind {
        SearchKind::Mojang => search_mojang(cli, args),
        SearchKind::Local => search_local(cli, args),
        SearchKind::Fabric => search_fabric(cli, args, &fabric::FABRIC_API),
        SearchKind::Quilt => search_fabric(cli, args, &fabric::QUILT_API),
        SearchKind::LegacyFabric => search_fabric(cli, args, &fabric::LEGACY_FABRIC_API),
        SearchKind::Babric => search_fabric(cli, args, &fabric::BABRIC_API),
        SearchKind::Forge => todo!(),
        SearchKind::NeoForge => todo!(),
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

    // Initial requests...
    let mut handler = CommonHandler::new(&mut cli.out);
    let manifest = match Manifest::request(handler.as_download_dyn()) {
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
    let only_name = if let Some(latest_channel) = args.latest {
        let name = match latest_channel {
            SearchLatestChannel::Release => manifest.name_of_latest(VersionChannel::Release),
            SearchLatestChannel::Snapshot => manifest.name_of_latest(VersionChannel::Snapshot),
        };
        if let Some(name) = name {
            Some(name)
        } else {
            return ExitCode::SUCCESS;
        }
    } else {
        None
    };

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
        
        let is_latest = manifest.name_of_latest(version.channel())
            .map(|name| name == version.name())
            .unwrap_or(false);

        let (channel_id, channel_fmt) = match version.channel() {
            VersionChannel::Release => ("release", "Release"),
            VersionChannel::Snapshot => ("snapshot", "Snapshot"),
            VersionChannel::Beta => ("beta", "Beta"),
            VersionChannel::Alpha => ("alpha", "Alpha"),
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

fn search_fabric(cli: &mut Cli, args: &SearchArgs, api: &fabric::Api) -> ExitCode {

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
