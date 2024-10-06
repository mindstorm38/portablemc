//! Various utilities to ease outputting human or machine readable text.

use std::io::{IsTerminal, StdoutLock, Write};
use std::time::{Duration, Instant};
use std::fmt::Display;
use std::{env, io};


/// An abstraction for outputting to any format on stdout, the goal is to provide an
/// interface for outputting at the same time both human readable and machine outputs.
#[derive(Debug)]
pub struct Output {
    /// Mode-specific data.
    mode: OutputMode,
    /// Is color enabled or disabled.
    color: bool,
}

#[derive(Debug)]
enum OutputMode {
    Human {
        log_level: LogLevel,
    },
    TabSeparated {  },
}

impl Output {

    pub fn human(log_level: LogLevel) -> Self {
        Self::new(OutputMode::Human { log_level })
    }

    pub fn tab_separated() -> Self {
        Self::new(OutputMode::TabSeparated {  })
    }

    fn new(mode: OutputMode) -> Self {
        Self {
            mode,
            color: has_stdout_color(),
        }
    }

    /// Enter log mode, this is exclusive with other modes.
    pub fn log(&mut self) -> LogOutput {
        LogOutput {
            output: self,
        }
    }

    /// Enter table mode, this is exclusive with other modes.
    pub fn table(&mut self) -> () {
        todo!()
    }

}

/// The output log mode, used to log events and various other messages, with an optional
/// state associated and possibly re-writable line for human readable output.
#[derive(Debug)]
pub struct LogOutput<'a> {
    /// Exclusive access to output.
    output: &'a mut Output,
}

impl<'o> LogOutput<'o> {

    /// Log an information with a simple code referencing it.
    pub fn log(&mut self, code: &str) -> Log<'_, false> {

        let mut writer = io::stdout().lock();

        if let OutputMode::TabSeparated {  } = self.output.mode {
            write!(writer, "{code}").unwrap();
        }

        Log {
            output: &mut self.output,
            writer,
            has_message: false,
        }

    }

    /// A special log type that is interpreted as a background task, on machine readable
    /// outputs it acts as a regular log, but on human-readable outputs it will be 
    /// displayed at the end of the current line.
    #[inline]
    pub fn background_log(&mut self, code: &str) -> Log<'_, true> {

        let mut writer = io::stdout().lock();

        if let OutputMode::TabSeparated {  } = self.output.mode {
            write!(writer, "{code}").unwrap();
        }

        Log {
            output: &mut self.output,
            writer,
            has_message: false,
        }

    }

}

/// A handle to a log line, allows adding more context to the log.
#[derive(Debug)]
pub struct Log<'a, const BG: bool> {
    /// Exclusive access to output.
    output: &'a mut Output,
    /// Locked writer.
    writer: StdoutLock<'static>,
    /// Set to true after the first human-readable message was written.
    has_message: bool,
}

impl<const BG: bool> Log<'_, BG> {

    /// Append an argument for machine-readable output.
    pub fn arg<D: Display>(&mut self, arg: D) -> &mut Self {
        if let OutputMode::TabSeparated {  } = self.output.mode {
            write!(self.writer, "\t{arg}").unwrap();
        }
        self
    }

    /// Append many arguments for machine-readable output.
    pub fn args<D, I>(&mut self, args: I) -> &mut Self
    where
        I: Iterator<Item = D>,
        D: Display,
    {
        if let OutputMode::TabSeparated {  } = self.output.mode {
            for arg in args {
                write!(self.writer, "\t{arg}").unwrap();
            }
        }
        self
    }

}

impl Log<'_, false> {

    /// Associate a human-readable message to this with an associated level, level is
    /// only relevant here because machine-readable outputs are always verbose.
    /// 
    /// If multiple message are written, only the first message will overwrite the 
    /// current line, and the .
    pub fn line<D: Display>(&mut self, level: LogLevel, message: D) -> &mut Self {
        if let OutputMode::Human { log_level } = self.output.mode {
            if level >= log_level {

                let (name, color) = match level {
                    LogLevel::Info => ("INFO", "\x1b[34m"),
                    LogLevel::Progress => ("..", ""),
                    LogLevel::Success => ("OK", "\x1b[92m"),
                    LogLevel::Warning => ("WARN", "\x1b[33m"),
                    LogLevel::Error => ("FAILED", "\x1b[31m"),
                };

                // \r      got to line start
                // \x1b[K  clear the whole line
                if !self.output.color || color.is_empty() {
                    write!(self.writer, "\r\x1b[K[{name:^6}] {message}").unwrap();
                } else {
                    write!(self.writer, "\r\x1b[K[{color}{name:^6}\x1b[0m] {message}").unwrap();
                }

                // If not a progress level, do a line return.
                if level != LogLevel::Progress {
                    self.writer.write_all(b"\n").unwrap();
                }

                self.has_message = true;

            }
        }
        self
    }

    #[inline]
    pub fn info<D: Display>(&mut self, message: D) -> &mut Self {
        self.line(LogLevel::Info, message)
    }

    #[inline]
    pub fn progress<D: Display>(&mut self, message: D) -> &mut Self {
        self.line(LogLevel::Progress, message)
    }

    #[inline]
    pub fn success<D: Display>(&mut self, message: D) -> &mut Self {
        self.line(LogLevel::Success, message)
    }

    #[inline]
    pub fn warning<D: Display>(&mut self, message: D) -> &mut Self {
        self.line(LogLevel::Warning, message)
    }

    #[inline]
    pub fn error<D: Display>(&mut self, message: D) -> &mut Self {
        self.line(LogLevel::Error, message)
    }

}

impl Log<'_, true> {
    
    /// Set the human-readable message of this background log. Note that this will 
    /// overwrite any background message currently written on the current log line.
    pub fn message<D: Display>(&mut self, message: D) -> &mut Self {
        if let OutputMode::Human { .. } = self.output.mode {
            // \x1b[u: restore saved cursor position
            write!(self.writer, "\x1b[u{message}").unwrap();
        }
        self
    }

}

/// Drop implementation to automatically flush the line, and optionally rewrite the 
/// suffix.
impl<const BACKGROUND: bool> Drop for Log<'_, BACKGROUND> {
    fn drop(&mut self) {

        // Save the position of the cursor at the end of the line, this is used to
        // easily rewrite the background task.
        if let OutputMode::Human { .. } = self.output.mode {
            if !BACKGROUND && self.has_message {
                // \x1b[s  save current cursor position
                self.writer.write_all(b"\x1b[s").unwrap();
            }
        } else {
            // Not in human-readable mode, line return anyway.
            self.writer.write_all(b"\n").unwrap();
        }

        self.writer.flush().unwrap();

    }
}

/// Level for a human-readable log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// This log is something indicative, discarded when not in verbose mode.
    Info,
    /// This log indicate something is in progress and the definitive state is unknown.
    Progress,
    /// This log indicate a success.
    Success,
    /// This log is a warning.
    Warning,
    /// This log is an error.
    Error,
}


/// A common download handler to compute various metrics.
#[derive(Debug)]
pub struct DownloadTracker {
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
}

#[derive(Debug)]
pub struct DownloadMetrics {
    /// Elapsed time since download started.
    pub elapsed: Duration,
    /// Average speed since download started (bytes/s).
    pub speed: f32,
}

impl DownloadTracker {

    pub fn new() -> Self {
        Self { download_start: None }
    }

    /// Handle progress of a download, returning some metrics if computable.
    pub fn handle(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) -> Option<DownloadMetrics> {

        let _ = total_size;
        
        if self.download_start.is_none() {
            self.download_start = Some(Instant::now());
        }

        if size == 0 {
            if count == total_count {
                // If all entries have been downloaded but the weight nothing, reset the
                // download start. This is possible with zero-sized files or cache mode.
                self.download_start = None;
            }
            return None;
        }

        let elapsed = self.download_start.unwrap().elapsed();
        let speed = size as f32 / elapsed.as_secs_f32();

        if count == total_count {
            self.download_start = None;
        }

        Some(DownloadMetrics {
            elapsed,
            speed,
        })

    }

}


/// Return true if color should be used on terminal.
/// 
/// Supporting `NO_COLOR` (https://no-color.org/) and `TERM=dumb`.
fn has_color() -> bool {
    if cfg!(unix) && env::var_os("TERM").map(|term| term == "dumb").unwrap_or_default() {
        false
    } else if env::var_os("NO_COLOR").map(|s| !s.is_empty()).unwrap_or_default() {
        false
    } else {
        true
    }
}

/// Return true if color can be printed to stdout.
/// 
/// See [`has_color()`].
fn has_stdout_color() -> bool {
    if !io::stdout().is_terminal() {
        false
    } else {
        has_color()
    }
}

/// Find the SI unit of a given number and return the number scaled down to that unit.
pub fn number_si_unit(num: f32) -> (f32, char) {
    match num {
        ..=999.0 => (num, ' '),
        ..=999_999.0 => (num / 1_000.0, 'k'),
        ..=999_999_999.0 => (num / 1_000_000.0, 'M'),
        _ => (num / 1_000_000_000.0, 'G'),
    }
}

// /// Compute terminal display length of a given string.
// fn terminal_width(s: &str) -> usize {

//     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//     enum Control {
//         None,
//         Escape,
//         Csi,
//     }

//     let mut width = 0;
//     let mut control = Control::None;

//     for ch in s.chars() {
//         match (control, ch) {
//             (Control::None, '\x1b') => {
//                 control = Control::Escape;
//             }
//             (Control::None, c) if !c.is_control() => {
//                 width += 1;
//             }
//             (Control::Escape, '[') => {
//                 control = Control::Csi;
//             }
//             (Control::Escape, _) => {
//                 control = Control::None;
//             }
//             (Control::Csi, c) if c.is_alphabetic() => {
//                 // After a CSI control any alphabetic char is terminating the sequence.
//                 control = Control::None;
//             }
//             _ => {}
//         }
//     }

//     width

// }


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn check_terminal_width() {
        assert_eq!(terminal_width(""), 0);
        assert_eq!(terminal_width("\x1b"), 0);
        assert_eq!(terminal_width("\x1b[92m"), 0);
        assert_eq!(terminal_width("\x1b[92mOK"), 2);
        assert_eq!(terminal_width("[  \x1b[92mOK"), 5);
        assert_eq!(terminal_width("[  \x1b[92mOK  ]"), 8);
        assert_eq!(terminal_width("[  \x1b[92mOK  \x1b[0m]"), 8);
    }

}
