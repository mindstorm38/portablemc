//! Implementation of the 'search' command.

use std::process::ExitCode;
use std::fs;

use chrono::{DateTime, Local, TimeDelta, Utc};
use portablemc::{mojang, standard};
use portablemc::download::Handler;

use crate::parse::{SearchArgs, SearchKind};
use crate::format::TimeDeltaDisplay;

use super::{Cli, CommonHandler, log_standard_error, log_io_error};


pub fn main(cli: &mut Cli, args: &SearchArgs) -> ExitCode {
    
    match args.kind {
        SearchKind::Mojang => search_mojang(cli, &args.query),
        SearchKind::Local => search_local(cli, &args.query),
        SearchKind::Forge => todo!(),
        SearchKind::Fabric => todo!(),
        SearchKind::Quilt => todo!(),
        SearchKind::LegacyFabric => todo!(),
    }

}

fn search_mojang(cli: &mut Cli, query: &[String]) -> ExitCode {

    // Initial requests...
    let mut handler = CommonHandler::new(&mut cli.out);
    let manifest = match mojang::request_manifest(handler.as_download_dyn()) {
        Ok(manifest) => manifest,
        Err(e) => {
            log_standard_error(&mut cli.out, e);
            return ExitCode::FAILURE;
        }
    };

    let today = Utc::now();

    // Parse the query.
    let mut filter_strings = Vec::new(); 
    let mut filter_type = Vec::new();
    let mut only_one = None;
    for part in query {
        if let Some((param, value)) = part.split_once(":") {
            match param {
                "type" => {
                    filter_type.push(match value {
                        "release" => standard::serde::VersionType::Release,
                        "snapshot" => standard::serde::VersionType::Snapshot,
                        "beta" => standard::serde::VersionType::OldBeta,
                        "alpha" => standard::serde::VersionType::OldAlpha,
                        _ => {
                            cli.out.log("error_invalid_type_param")
                                .arg(value)
                                .error(format_args!("Unknown type: {value}"));
                            return ExitCode::FAILURE;
                        }
                    });
                }
                "release" => {
                    if !value.is_empty() {
                        cli.out.log("error_invalid_release_param")
                            .error("Param 'release' don't expect value");
                        return ExitCode::FAILURE;
                    } else if let Some(id) = manifest.latest.get(&standard::serde::VersionType::Release) {
                        only_one = Some(id.clone());
                    }
                }
                "snapshot" => {
                    if !value.is_empty() {
                        cli.out.log("error_invalid_snapshot_param")
                            .error("Param 'snapshot' don't expect value");
                        return ExitCode::FAILURE;
                    } else if let Some(id) = manifest.latest.get(&standard::serde::VersionType::Snapshot) {
                        only_one = Some(id.clone());
                    }
                }
                _ => {
                    cli.out.log("error_unknown_param")
                        .arg(param)
                        .arg(value)
                        .error(format_args!("Unknown param: '{part}'"));
                    return ExitCode::FAILURE;
                }
            }
        } else {
            filter_strings.push(part.as_str());
        }
    }

    // Now we construct the table...
    let mut table = cli.out.table(3);

    {
        let mut row = table.row();
        row.cell("id").format("Identifier");
        row.cell("type").format("Type");
        row.cell("release_date").format("Release date");
    }
    
    table.sep();

    for version in &manifest.versions {

        if let Some(only_one) = only_one.as_deref() {
            if version.id != only_one {
                continue;
            }
        } else {
            if !filter_strings.is_empty() {
                if !filter_strings.iter().any(|s| version.id.contains(s)) {
                    continue;
                }
            }

            if !filter_type.is_empty() {
                if !filter_type.contains(&version.r#type) {
                    continue;
                }
            }
        }
        
        let mut row = table.row();
        row.cell(&version.id);
        
        let is_latest = manifest.latest.get(&version.r#type)
            .map(|id| id == &version.id)
            .unwrap_or(false);

        let (type_id, type_fmt) = match version.r#type {
            standard::serde::VersionType::Release => ("release", "Release"),
            standard::serde::VersionType::Snapshot => ("snapshot", "Snapshot"),
            standard::serde::VersionType::OldBeta => ("beta", "Beta"),
            standard::serde::VersionType::OldAlpha => ("alpha", "Alpha"),
        };
        
        if is_latest {
            row.cell(format_args!("{type_id}*")).format(format_args!("{type_fmt}*"));
        } else {
            row.cell(format_args!("{type_id}")).format(format_args!("{type_fmt}"));
        }

        let mut cell = row.cell(&version.release_time.to_rfc3339());
        let local_release_date = version.release_time.with_timezone(&Local);
        let local_release_data_fmt: _ = version.release_time.format("%a %b %e %T %Y");

        let delta = today.signed_duration_since(&local_release_date);
        if is_latest || delta <= TimeDelta::weeks(4) {
            cell.format(format_args!("{} ({})", local_release_data_fmt, TimeDeltaDisplay(delta)));
        } else {
            cell.format(format_args!("{}", local_release_data_fmt));
        }

    }

    ExitCode::SUCCESS

}

fn search_local(cli: &mut Cli, query: &[String]) -> ExitCode {

    let reader = match fs::read_dir(&cli.versions_dir) {
        Ok(reader) => reader,
        Err(e) => {
            log_io_error(&mut cli.out, e, Some(&cli.versions_dir));
            return ExitCode::FAILURE;
        }
    };

    // Parse the query.
    let mut filter_strings = Vec::new(); 
    for part in query {
        if let Some((param, value)) = part.split_once(":") {
            match param {
                _ => {
                    cli.out.log("error_unknown_param")
                        .arg(param)
                        .arg(value)
                        .error(format_args!("Unknown param: '{part}'"));
                    return ExitCode::FAILURE;
                }
            }
        } else {
            filter_strings.push(part.as_str());
        }
    }
    
    // Construct the table.
    let mut table = cli.out.table(2);

    {
        let mut row = table.row();
        row.cell("id").format("Identifier");
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

        if !filter_strings.is_empty() {
            if !filter_strings.iter().any(|s| version_id.contains(s)) {
                continue;
            }
        }
        
        let mut row = table.row();
        row.cell(&version_id);
        row.cell(&version_last_modified.to_rfc3339())
            .format(version_last_modified.format("%a %b %e %T %Y"));

    }

    ExitCode::SUCCESS

}
