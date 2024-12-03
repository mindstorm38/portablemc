//! Implementation of the 'start' command.

use std::process::{Child, Command, ExitCode, Stdio};
use std::io::{self, BufRead, BufReader};
use std::sync::Mutex;

use chrono::{DateTime, Local, Utc};

use portablemc::mojang::{self, Handler as _};
use portablemc::standard;

use crate::parse::{StartArgs, StartResolution, StartVersion};
use crate::format::TIME_FORMAT;
use crate::output::LogLevel;

use super::{Cli, CommonHandler, log_mojang_error, log_io_error};


/// The child is shared in order to be properly killed when the launcher exits, because
/// it's not the case on Windows by default.
pub static GAME_CHILD: Mutex<Option<Child>> = Mutex::new(None);


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

    match run_game(cli, command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log_io_error(&mut cli.out, e, None);
            ExitCode::FAILURE
        }
    }

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

/// Internal function to run the game, separated in order to catch I/O errors.
fn run_game(cli: &mut Cli, mut command: Command) -> io::Result<()> {

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
    // release the guard so any handled CTRL+C will terminate it.
    let mut pipe = BufReader::new(child.stdout.take().unwrap());
    *child_guard = Some(child);
    drop(child_guard);

    let mut buffer = Vec::new();
    let mut xml = None::<XmlLogParser>;
    let mut child_guard = None;

    // Read line by line, but not into a string because we don't really know if the 
    // output will be UTF-8 compliant, so we store raw bytes in the buffer.
    while pipe.read_until(b'\n', &mut buffer)? != 0 {

        // We keep the buffer trim in case of XML, to avoid a useless text event.
        let buffer_trim = buffer.trim_ascii();
        let Ok(buffer_str) = std::str::from_utf8(buffer_trim) else { 
            buffer.clear();
            continue
        };

        if xml.is_none() && buffer_str.starts_with("<log4j:") {
            xml = Some(XmlLogParser::default());
        }

        // In case of XML we try to decode it, if it's successful.
        let mut xml_valid = false;
        if let Some(parser) = &mut xml {
            if let Some(logs) = parser.feed(buffer_str) {

                xml_valid = true;

                for xml_log in logs {
            
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

            }
        }

        if !xml_valid {
            xml = None;
        }

        if xml.is_none() {

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

        // We don't really know if this line will execute in case of a CTRL+C, which will
        // take the child to kill it itself, so it might be absent here. We also put it
        // in an option that allows us to keep the guard for the .wait after the loop.
        let guard: _ = child_guard.insert(GAME_CHILD.lock().unwrap());
        let Some(child) = guard.as_mut() else { break };

        // If child is terminated, we take the 
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


#[derive(Debug, Default)]
struct XmlLogParser {
    /// Queue of logs returned when fully parsed.
    logs: Vec<XmlLog>,
    /// The current state, or tag, we are decoding.
    state: XmlLogState,
    /// True when the current state is still decoding its attributes.
    state_attributes: bool,
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
    pub fn feed(&mut self, buffer: &str) -> Option<impl Iterator<Item = XmlLog> + '_> {

        use xmlparser::{Tokenizer, Token, ElementEnd};

        for token in Tokenizer::from_fragment(buffer, 0..buffer.len()) {
            
            let Ok(token) = token else { return None };

            match token {
                Token::ElementStart { prefix, local, .. } => {
                    
                    if self.state_attributes {
                        return None;
                    }

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

                    self.state_attributes = true;

                }
                Token::ElementEnd { end: ElementEnd::Close(prefix, local), .. } => {

                    if self.state_attributes {
                        return None;
                    }

                    match (self.state, &*prefix, &*local) {
                        (XmlLogState::Event, "log4j", "Event") => {
                            self.state = XmlLogState::None;
                        }
                        (XmlLogState::Event, _, _) => return None,
                        (XmlLogState::Message, "log4j", "Message") => {
                            self.state = XmlLogState::Event;
                        }
                        (XmlLogState::Message, _, _) => return None,
                        (XmlLogState::Throwable, "log4j", "Throwable") => {
                            self.state = XmlLogState::Event;
                        }
                        (XmlLogState::Throwable, _, _) => return None,
                        _ => continue,
                    }

                }
                Token::ElementEnd { .. } => {
                    if !self.state_attributes {
                        return None;
                    }
                    self.state_attributes = false;
                    
                }
                Token::Attribute { local, prefix, value, .. } => {

                    if !self.state_attributes {
                        return None;
                    } else if self.state != XmlLogState::Event {
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
                                _ => return None,
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

                    if self.state_attributes {
                        return None;
                    } else if self.state == XmlLogState::None {
                        continue;
                    }
                    
                    let log = self.logs.last_mut().unwrap();

                    match self.state {
                        XmlLogState::Message => log.message = text.to_string(),
                        XmlLogState::Throwable => log.message = text.to_string(),
                        _ => continue,
                    }

                }
                _ => continue,
            }

        }

        if self.state != XmlLogState::None {
            Some(self.logs.drain(..self.logs.len() - 1))
        } else {
            Some(self.logs.drain(..))
        }
        
    }

}
