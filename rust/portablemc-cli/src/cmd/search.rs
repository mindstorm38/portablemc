//! Implementation of the 'search' command.

use std::process::ExitCode;
use std::fs;

use chrono::{Local, TimeDelta, Utc};
use portablemc::download::Handler;
use portablemc::{mojang, standard};

use crate::parse::{SearchArgs, SearchKind};
use crate::format::TimeDeltaDisplay;

use super::{Cli, CommonHandler, log_standard_error};


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
    let mut handler = CommonHandler::new(cli.out.logger());
    let manifest = match mojang::request_manifest(handler.as_download_dyn()) {
        Ok(manifest) => manifest,
        Err(e) => {
            log_standard_error(&mut handler.logger, e);
            return ExitCode::FAILURE;
        }
    };

    let today = Utc::now();

    // Parse the query.
    let mut logger = cli.out.logger();
    let mut filter_strings = Vec::new(); 
    let mut filter_type = Vec::new();
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
                            logger.log("error_invalid_type_param")
                                .arg(value)
                                .error(format_args!("Unknown type: {value}"));
                            return ExitCode::FAILURE;
                        }
                    });
                }
                "release" => {
                    if !value.is_empty() {
                        logger.log("error_invalid_release_param")
                            .error("Param 'release' don't expect value");
                        return ExitCode::FAILURE;
                    } else if let Some(id) = manifest.latest.get(&standard::serde::VersionType::Release) {
                        filter_strings = vec![id.as_str()];
                        filter_type = vec![standard::serde::VersionType::Release];
                        break;
                    }
                }
                "snapshot" => {
                    if !value.is_empty() {
                        logger.log("error_invalid_snapshot_param")
                            .error("Param 'snapshot' don't expect value");
                        return ExitCode::FAILURE;
                    } else if let Some(id) = manifest.latest.get(&standard::serde::VersionType::Snapshot) {
                        filter_strings = vec![id.as_str()];
                        filter_type = vec![standard::serde::VersionType::Snapshot];
                        break;
                    }
                }
                _ => {
                    logger.log("error_unknown_param")
                        .arg(param)
                        .arg(value)
                        .error(format_args!("Unknown param: {part}"));
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
            return ExitCode::FAILURE;
        }
    };

    for entry in reader {
        
    }

    ExitCode::SUCCESS

}
