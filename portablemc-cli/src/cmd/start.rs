//! Implementation of the 'start' command.

use std::process::{Child, Command, ExitCode, Stdio};
use std::io::{self, BufRead, BufReader};
use std::sync::Mutex;

use chrono::{DateTime, Local, Utc};

use portablemc::moj::{self, FetchExclude, QuickPlay};
use portablemc::base::{self, Game, JvmPolicy};
use portablemc::{fabric, forge};

use crate::parse::{StartArgs, StartResolution, StartVersion, StartJvmPolicy};
use crate::format::TIME_FORMAT;
use crate::output::LogLevel;

use super::{Cli, LogHandler, log_any_error, log_mojang_error, log_fabric_error, log_forge_error, log_msa_database_error};


/// The child is shared in order to be properly killed when the launcher exits, because
/// it's not the case on Windows by default.
pub static GAME_CHILD: Mutex<Option<Child>> = Mutex::new(None);


pub fn start(cli: &mut Cli, args: &StartArgs) -> ExitCode {

    match args.version {
        StartVersion::Mojang { 
            ref version,
        } => {
            start_mojang(version.clone(), cli, args)
        }
        StartVersion::MojangRelease |
        StartVersion::MojangSnapshot => {

            let handler = LogHandler::new(&mut cli.out);
            let repo = match moj::Manifest::request(handler) {
                Ok(repo) => repo,
                Err(e) => {
                    log_mojang_error(cli, &e);
                    return ExitCode::FAILURE;
                }
            };

            let version = match args.version {
                StartVersion::MojangRelease => repo.latest_release_name(),
                StartVersion::MojangSnapshot => repo.latest_snapshot_name(),
                _ => unreachable!(),
            };

            start_mojang(version.to_string(), cli, args)

        }
        StartVersion::Fabric { 
            loader,
            ref game_version, 
            ref loader_version, 
        } => {
            start_fabric(loader, game_version.clone(), loader_version.clone(), cli, args)
        }
        StartVersion::Forge { 
            loader, 
            ref version,
        } => {
            start_forge(loader, version.clone().into(), cli, args)
        }
        StartVersion::ForgeLatest { 
            loader, 
            ref game_version, 
            stable,
        } => {
            
            let game_version = match game_version {
                Some(game_version) => game_version.clone(),
                None => {
                    
                    let handler = LogHandler::new(&mut cli.out);
                    let manifest = match moj::Manifest::request(handler) {
                        Ok(repo) => repo,
                        Err(e) => {
                            log_mojang_error(cli, &e);
                            return ExitCode::FAILURE;
                        }
                    };

                    manifest.latest_release_name().to_string()

                }
            };

            let version = if stable {
                forge::Version::Stable(game_version)
            } else {
                forge::Version::Unstable(game_version)
            };

            start_forge(loader, version, cli, args)

        }
    }

}

/// Main entrypoint for starting a Mojang version from its name.
fn start_mojang(
    version: String, 
    cli: &mut Cli, 
    args: &StartArgs,
) -> ExitCode {

    let mut inst = moj::Installer::new(version);
    if !apply_mojang_args(&mut inst, &mut *cli, args) {
        return ExitCode::FAILURE;
    }

    let log_handler = LogHandler::new(&mut cli.out);
    let start_handler = StartHandler::new(args, log_handler);

    match inst.install(start_handler) {
        Ok(game) => start_game(game, cli, args),
        Err(e) => {
            log_mojang_error(cli, &e);
            return ExitCode::FAILURE;
        }
    }
    
}

/// Main entrypoint for starting a Fabric-based version.
fn start_fabric(
    loader: fabric::Loader, 
    game_version: fabric::GameVersion, 
    loader_version: fabric::LoaderVersion,
    cli: &mut Cli, 
    args: &StartArgs,
) -> ExitCode {

    let mut inst = fabric::Installer::new(loader, game_version.clone(), loader_version.clone());
    if !apply_mojang_args(inst.mojang_mut(), &mut *cli, args) {
        return ExitCode::FAILURE;
    }
    
    let mut log_handler = LogHandler::new(&mut cli.out);
    log_handler.set_fabric_loader(loader);
    let start_handler = StartHandler::new(args, log_handler);

    match inst.install(start_handler) {
        Ok(game) => start_game(game, cli, args),
        Err(e) => {
            log_fabric_error(cli, &e, loader);
            return ExitCode::FAILURE;
        }
    }

}

/// Main entrypoint for starting a Forge/NeoForge version.
fn start_forge(
    loader: forge::Loader, 
    version: forge::Version, 
    cli: &mut Cli, 
    args: &StartArgs,
) -> ExitCode {

    let mut inst = forge::Installer::new(loader, version);
    if !apply_mojang_args(inst.mojang_mut(), &mut *cli, args) {
        return ExitCode::FAILURE;
    }

    let mut log_handler = LogHandler::new(&mut cli.out);
    log_handler.set_forge_loader(inst.loader());
    let start_handler = StartHandler::new(args, log_handler);
    
    match inst.install(start_handler) {
        Ok(game) => start_game(game, cli, args),
        Err(e) => {
            log_forge_error(cli, &e, inst.loader());
            return ExitCode::FAILURE;
        }
    }

}

/// Main entrypoint for running the installed game.
fn start_game(game: Game, cli: &mut Cli, args: &StartArgs) -> ExitCode {

    // Build the command here so that we can debug it's arguments without launching.
    let command = game.command();
    {
        let mut log = cli.out.log("jvm_args");
        log.args(command.get_args().filter_map(|a| a.to_str()));
        log.info("Arguments:");
        for arg in command.get_args().filter_map(|a| a.to_str()) {
            log.additional(arg);
        }
    }

    if args.dry {
        return ExitCode::SUCCESS;
    }

    match run_command(cli, command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {

            cli.out.log("error_run_command")
                .error("Failed to run command");

            log_any_error(cli, &e, false, true);
            ExitCode::FAILURE
            
        }
    }

}

// Internal function to apply args to the base installer.
fn apply_base_args(
    installer: &mut base::Installer, 
    cli: &mut Cli, 
    args: &StartArgs,
) -> bool {

    // installer.set_versions_dir(cli.versions_dir.clone());
    // installer.set_libraries_dir(cli.libraries_dir.clone());
    // installer.set_assets_dir(cli.assets_dir.clone());
    // installer.set_jvm_dir(cli.jvm_dir.clone());
    // installer.set_bin_dir(cli.bin_dir.clone());
    // installer.set_mc_dir(cli.mc_dir.clone());

    installer.set_main_dir(cli.main_dir.clone());

    if let Some(jvm_file) = &args.jvm {
        installer.set_jvm_policy(JvmPolicy::Static(jvm_file.into()));
    } else {
        installer.set_jvm_policy(match args.jvm_policy {
            StartJvmPolicy::System => JvmPolicy::System,
            StartJvmPolicy::Mojang => JvmPolicy::Mojang,
            StartJvmPolicy::SystemThenMojang => JvmPolicy::SystemThenMojang,
            StartJvmPolicy::MojangThenSystem => JvmPolicy::MojangThenSystem,
        });
    }

    true

}

// Internal function to apply args to the mojang installer.
fn apply_mojang_args(
    installer: &mut moj::Installer,
    cli: &mut Cli, 
    args: &StartArgs,
) -> bool {

    // FIXME: For now, the telemetry client id is kept unset.

    if args.auth {

        let res =
        if let Some(uuid) = args.uuid {
            
            if args.username.is_some() {
                cli.out.log("warn_username_ignored")
                    .warning("You specified both '--uuid' (-i) and '--username' (-u) with '--auth' (-a), so '--username' will be ignored");
            }

            cli.msa_db.load_from_uuid(uuid)

        } else if let Some(username) = &args.username {
            cli.msa_db.load_from_username(&username)
        } else {
            
            cli.out.log("error_missing_auth_uuid_or_username")
                .error("Missing '--uuid' (-i) or '--username' (-u), required when using '--auth' (-a)");

            return false;

        };

        let mut account = match res {
            Ok(Some(account)) => account,
            Ok(None) => {

                let mut log = cli.out.log("error_account_not_found");

                if let Some(uuid) = args.uuid {
                    log.arg(&uuid);
                    log.error(format_args!("No account found for: {uuid}"));
                } else if let Some(username) = &args.username {
                    log.arg(&username);
                    log.error(format_args!("No account found for username: {username}"));
                } else {
                    unreachable!();
                }

                log.additional(format_args!("Use 'portablemc auth login' command to log into your account"));
                log.additional(format_args!("Use 'portablemc auth list' to list stored accounts"));
                return false;
                
            }
            Err(error) => {
                log_msa_database_error(cli, &error);
                return false;
            }
        };

        // Refresh account before setting auth to the installer, in case tokens changed.
        if !super::auth::refresh_account(&mut *cli, &mut account, true) {
            return false;
        }

        installer.set_auth_msa(&account);

    } else {
        match (&args.username, args.uuid) {
            (Some(username), None) => 
                installer.set_auth_offline_username(username.clone()),
            (None, Some(uuid)) =>
                installer.set_auth_offline_uuid(uuid),
            (Some(username), Some(uuid)) =>
                installer.set_auth_offline(uuid, username.clone()),
            (None, None) => installer, // nothing
        };
    }

    if !apply_base_args(installer.base_mut(), &mut *cli, args) {
        return false;
    }

    installer.set_disable_multiplayer(args.disable_multiplayer);
    installer.set_disable_chat(args.disable_chat);
    installer.set_demo(args.demo);

    if let Some(StartResolution { width, height }) = args.resolution {
        installer.set_resolution(width, height);
    }

    installer.set_fix_legacy_quick_play(!args.no_fix_legacy_quick_play);
    installer.set_fix_legacy_proxy(!args.no_fix_legacy_proxy);
    installer.set_fix_legacy_merge_sort(!args.no_fix_legacy_merge_sort);
    installer.set_fix_legacy_resolution(!args.no_fix_legacy_resolution);
    installer.set_fix_broken_authlib(!args.no_fix_broken_authlib);

    if let Some(lwjgl_version) = &args.fix_lwjgl {
        installer.set_fix_lwjgl(lwjgl_version.to_string());
    }

    if args.fetch_exclude_all {
        installer.add_fetch_exclude(FetchExclude::All);
    } else {
        // NOTE: For now we don't support regex patterns!
        for exclude_name in &args.fetch_exclude {
            installer.add_fetch_exclude(FetchExclude::Exact(exclude_name.clone()));
        }
    }

    if let Some(name) = &args.join_world {
        installer.set_quick_play(QuickPlay::Singleplayer { name: name.clone() });
    } else if let Some(host) = &args.join_server {
        installer.set_quick_play(QuickPlay::Multiplayer { 
            host: host.clone(), 
            port: args.join_server_port,
        });
    } else if let Some(id) = &args.join_realms {
        installer.set_quick_play(QuickPlay::Realms { id: id.clone() });
    }

    true

}

/// Internal function to run the game, separated in order to catch I/O errors.
fn run_command(cli: &mut Cli, mut command: Command) -> io::Result<()> {

    // Keep the guard while we are launching the command.
    let mut child_guard = GAME_CHILD.lock().unwrap();
    assert!(child_guard.is_none(), "more than one game run at a time");

    cli.out.log("launching")
        .pending("Launching...");

    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());

    let mut child = command.spawn()?;

    cli.out.log("launched")
        .arg(child.id())
        .success("Launched");

    // Take the stdout pipe and put the child in the shared location, only then we
    // release the guard so any handled Ctrl-C will terminate it.
    let mut pipe = BufReader::new(child.stdout.take().unwrap());
    *child_guard = Some(child);
    drop(child_guard);

    let mut buffer = Vec::new();
    let mut xml = None::<XmlLogParser>;
    let mut child_guard = None;

    // Read line by line, but not into a string because we don't really know if the 
    // output will be UTF-8 compliant, so we store raw bytes in the buffer.
    while pipe.read_until(b'\n', &mut buffer)? != 0 {

        let Ok(buffer_str) = std::str::from_utf8(&buffer) else { 
            buffer.clear();
            continue
        };

        if xml.is_none() && buffer_str.trim_ascii_start().starts_with("<log4j:") {
            xml = Some(XmlLogParser::default());
        }

        // In case of XML we try to decode it, if it's successful.
        if let Some(parser) = &mut xml {
            for xml_log in parser.feed(buffer_str) {
        
                let xml_log_time = xml_log.time.with_timezone(&Local);
                
                let (level_code, level_name, log_level) = match xml_log.level {
                    XmlLogLevel::Trace => ("trace", "TRACE", LogLevel::Raw),
                    XmlLogLevel::Debug => ("debug", "DEBUG", LogLevel::Raw),
                    XmlLogLevel::Info => ("info", "INFO", LogLevel::Raw),
                    XmlLogLevel::Warn => ("warn", "WARN", LogLevel::RawWarn),
                    XmlLogLevel::Error => ("error", "ERROR", LogLevel::RawError),
                    XmlLogLevel::Fatal => ("fatal", "FATAL", LogLevel::RawFatal),
                };

                let mut log = cli.out.log("log_xml");
                log .arg(level_code)
                    .arg(xml_log_time.to_rfc3339())
                    .arg(&xml_log.logger)
                    .arg(&xml_log.thread)
                    .arg(&xml_log.message)
                    .line(log_level, format_args!("[{}] [{}] [{}] {}", 
                        xml_log_time.format(TIME_FORMAT),
                        xml_log.thread,
                        level_name,
                        xml_log.message));
                
                if let Some(throwable) = &xml_log.throwable {
                    log.line(LogLevel::RawError, format_args!("    {throwable}"));
                }

            }
        } else {

            let buffer_str = buffer_str.trim_ascii();

            let mut log_level = LogLevel::Raw;
            if buffer_str.contains("WARN") {
                log_level = LogLevel::RawWarn;
            } else if buffer_str.contains("ERROR") {
                log_level = LogLevel::RawError;
            } else if buffer_str.contains("SEVERE") || buffer_str.contains("FATAL") {
                log_level = LogLevel::RawFatal;
            }

            cli.out.log("log_raw")
                .arg(&buffer_str)
                .line(log_level, &buffer_str);
            
        }

        buffer.clear();

        // We don't really know if this line will execute in case of a Ctrl-C, which will
        // take the child to kill it itself, so it might be absent here. We also put it
        // in an option that allows us to keep the guard for the .wait after the loop.
        let guard: _ = child_guard.insert(GAME_CHILD.lock().unwrap());
        let Some(child) = guard.as_mut() else { break };

        // If child is terminated, we keep the guard and break.
        if child.try_wait()?.is_some() { 
            break;
        }

        // Release the guard if we continue the loop.
        drop(child_guard.take().unwrap());
        
    }

    // Do not lock again if we did in the loop before breaking...
    let guard: _ = child_guard.get_or_insert_with(|| GAME_CHILD.lock().unwrap());

    // This time we take the child because we will wait indefinitely on it.
    let Some(mut child) = guard.take() else {
        return Ok(());
    };

    // In the end, we'll only log that when the game is gently terminated.
    let status = child.wait()?;
    cli.out.log("terminated")
        .arg(status.code().unwrap_or_default())
        .info(format_args!("Terminated: {}", status.code().unwrap_or_default()));

    Ok(())

}

/// Internal structure used to continuously parse the stream of XML logs out of the game.
#[derive(Debug, Default)]
struct XmlLogParser {
    /// The buffer used to stack buffers while we have a parsing error at the end of it.
    buffer: String,
    /// Queue of logs returned when fully parsed.
    logs: Vec<XmlLog>,
    /// The current state, or tag, we are decoding.
    state: XmlLogState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum XmlLogState {
    #[default]
    None,
    Event,
    Message,
    Throwable,
}

#[derive(Debug, Default)]
struct XmlLog {
    logger: String,
    time: DateTime<Utc>,
    level: XmlLogLevel,
    thread: String,
    message: String,
    throwable: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum XmlLogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Fatal,
}

impl XmlLogParser {

    /// Feed the given buffer of tokens into the parser, any parsed log will be returned
    /// by the iterator. No iterator is returned if the parsing fails.
    pub fn feed(&mut self, buffer: &str) -> impl Iterator<Item = XmlLog> + use<'_> {

        use xmlparser::{Tokenizer, Token, ElementEnd};

        // Use the buffer instead of the input if required.
        let full_buffer = if !self.buffer.is_empty() {
            self.buffer.push_str(buffer);
            &*self.buffer
        } else {
            buffer
        };

        let mut tokenizer = Tokenizer::from_fragment(full_buffer, 0..full_buffer.len());
        let mut error = false;
        let mut last_pos = 0;

        for token in &mut tokenizer {
            
            let token = match token {
                Ok(token) => token,
                Err(_) => {

                    if self.buffer.is_empty() {
                        // If we are not yet using the buffer, initialize it.
                        self.buffer.push_str(&buffer[last_pos..]);
                    } else {
                        // If we did use the buffer, we need to cut all successful token.
                        self.buffer.drain(..last_pos);
                    }

                    error = true;
                    break;

                }
            };

            // Save the last position the tokenizer was successful, so we cut everything
            // up to this part in case of error.
            last_pos = token.span().start() + token.span().len();

            match token {
                Token::ElementStart { prefix, local, .. } => {
                    
                    match (self.state, &*prefix, &*local) {
                        (XmlLogState::None, "log4j", "Event") => {
                            // While we are not in None state, then we are operating on
                            // the last log of that vector.
                            self.logs.push(XmlLog::default());
                            self.state = XmlLogState::Event;
                        }
                        (XmlLogState::Event, "log4j", "Message") => {
                            self.state = XmlLogState::Message;
                        }
                        (XmlLogState::Event, "log4j", "Throwable") => {
                            self.state = XmlLogState::Throwable;
                        }
                        _ => continue,
                    }

                }
                Token::ElementEnd { end: ElementEnd::Close(prefix, local), .. } => {

                    match (self.state, &*prefix, &*local) {
                        (XmlLogState::Event, "log4j", "Event") => {
                            self.state = XmlLogState::None;
                        }
                        (XmlLogState::Event, _, _) => continue,
                        (XmlLogState::Message, "log4j", "Message") => {
                            self.state = XmlLogState::Event;
                        }
                        (XmlLogState::Message, _, _) => continue,
                        (XmlLogState::Throwable, "log4j", "Throwable") => {
                            self.state = XmlLogState::Event;
                        }
                        (XmlLogState::Throwable, _, _) => continue,
                        _ => continue,
                    }

                }
                Token::ElementEnd { .. } => { // For '>' or '/>'
                    continue;
                }
                Token::Attribute { local, prefix, value, .. } => {

                    if self.state != XmlLogState::Event {
                        continue;
                    }

                    // Valid because we are in event state, so the last log is built.
                    let log = self.logs.last_mut().unwrap();

                    match (&*prefix, &*local) {
                        ("", "logger") => {
                            log.logger = value.to_string();
                        }
                        ("", "timestamp") => {
                            let timestamp = value.parse::<i64>().unwrap_or(0);
                            log.time = DateTime::<Utc>::from_timestamp_millis(timestamp).unwrap();
                        }
                        ("", "level") => {
                            log.level = match &*value {
                                "TRACE" => XmlLogLevel::Trace,
                                "DEBUG" => XmlLogLevel::Debug,
                                "INFO" => XmlLogLevel::Info,
                                "WARN" => XmlLogLevel::Warn,
                                "ERROR" => XmlLogLevel::Error,
                                "FATAL" => XmlLogLevel::Fatal,
                                _ => continue,
                            };
                        }
                        ("", "thread") => {
                            log.thread = value.to_string();
                        }
                        _ => continue,
                    }

                }
                Token::Text { text } |
                Token::Cdata { text, .. } => {
                    
                    if self.state == XmlLogState::None {
                        continue;
                    }
                    
                    let log = self.logs.last_mut().unwrap();
                    let text = text.trim_ascii();

                    match self.state {
                        XmlLogState::Message => log.message = text.to_string(),
                        XmlLogState::Throwable => log.message = text.to_string(),
                        _ => continue,
                    }

                }
                _ => continue,
            }

        }

        if !error {
            // Clear the internal buffer, in case it was used and parsing was successful.
            self.buffer.clear();
        }

        if self.state != XmlLogState::None {
            self.logs.drain(..self.logs.len() - 1)
        } else {
            self.logs.drain(..)
        }
        
    }

}

/// The start handler that apply modifications to the game installation.
struct StartHandler<'a> {
    args: &'a StartArgs,
    log_handler: LogHandler<'a>,
}

impl<'a> StartHandler<'a> {

    pub fn new(args: &'a StartArgs, log_handler: LogHandler<'a>) -> Self {
        Self {
            args,
            log_handler,
        }
    }

    fn on_event_inner(&mut self, event: &mut base::Event) {
        match event {
            base::Event::FilterLibraries { libraries } => {

                if !self.args.exclude_lib.is_empty() {
                    libraries.retain(|lib| {
                        // If any pattern matches: .any(...) -> !true -> false (don't keep)
                        !self.args.exclude_lib.iter()
                            .any(|pattern| pattern.matches(&lib.name))
                    });
                }

            }
            base::Event::FilterLibrariesFiles { class_files, natives_files } => {

                class_files.extend_from_slice(&self.args.include_class);
                natives_files.extend_from_slice(&self.args.include_natives);

            }
            _ => {}
        }
    }

}

impl moj::Handler for StartHandler<'_> {
    fn on_event(&mut self, mut event: moj::Event) {
        
        if let moj::Event::Base(event) = &mut event {
            self.on_event_inner(event);
        }

        self.log_handler.on_event(event);

    }
}

impl fabric::Handler for StartHandler<'_> {
    fn on_event(&mut self, mut event: fabric::Event) {
        
        if let fabric::Event::Mojang(moj::Event::Base(event)) = &mut event {
            self.on_event_inner(event);
        }

        self.log_handler.on_event(event);

    }
}

impl forge::Handler for StartHandler<'_> {
    fn on_event(&mut self, mut event: forge::Event) {
        
        if let forge::Event::Mojang(moj::Event::Base(event)) = &mut event {
            self.on_event_inner(event);
        }

        self.log_handler.on_event(event);

    }
}
