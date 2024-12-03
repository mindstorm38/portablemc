//! Various utilities to ease outputting human or machine readable text.

use std::io::{IsTerminal, StdoutLock, Write};
use std::fmt::{self, Display, Write as _};
use std::{env, io};

use chrono::TimeDelta;


/// An abstraction for outputting to any format on stdout, the goal is to provide an
/// interface for outputting at the same time both human readable and machine outputs.
#[derive(Debug)]
pub struct Output {
    /// Mode-specific data.
    mode: OutputMode,
    /// Are cursor escape code supported on stdout.
    escape_cursor_cap: bool,
    /// Are color escape code supported on stdout.
    escape_color_cap: bool,
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

        let term_dumb = !io::stdout().is_terminal() || (cfg!(unix) && env::var_os("TERM").map(|term| term == "dumb").unwrap_or_default());
        let no_color = env::var_os("NO_COLOR").map(|s| !s.is_empty()).unwrap_or_default();

        Self {
            mode,
            escape_cursor_cap: !term_dumb,
            escape_color_cap: !term_dumb && !no_color,
        }
    }

    /// Enter log mode, this is exclusive with other modes.
    pub fn log(&mut self) -> LogOutput<'_> {

        // Save the initial cursor position for the first line to be written.
        if self.escape_cursor_cap {
            print!("\x1b[s");
        }

        LogOutput {
            output: self,
            shared: LogShared::default(),
        }
    }

    /// Enter table mode, this is exclusive with other modes.
    pub fn table(&mut self, columns: usize) -> TableOutput<'_> {
        assert_ne!(columns, 0);
        TableOutput {
            output: self,
            writer: io::stdout().lock(),
            columns,
            column: 0,
            buffer: String::new(),
            cells: Vec::new(),
        }
    }

}

/// The output log mode, used to log events and various other messages, with an optional
/// state associated and possibly re-writable line for human readable output.
#[derive(Debug)]
pub struct LogOutput<'a> {
    /// Exclusive access to output.
    output: &'a mut Output,
    /// Buffer storing the current background log message.
    shared: LogShared,
}

/// Internal buffer for the current line.
#[derive(Debug, Default)]
struct LogShared {
    /// Line buffer that will be printed when the log is dropped.
    line: String,
    /// For human-readable only, storing the rendered background log.
    background: String
}

impl<'o> LogOutput<'o> {

    fn _log<const BG: bool>(&mut self, code: &str) -> Log<'_, BG> {

        if let OutputMode::TabSeparated {  } = self.output.mode {
            debug_assert!(self.shared.line.is_empty());
            self.shared.line.push_str(code);
        }

        Log {
            output: &mut self.output,
            shared: &mut self.shared,
        }

    }

    /// Log an information with a simple code referencing it.
    #[inline]
    pub fn log(&mut self, code: &str) -> Log<'_, false> {
        self._log(code)
    }

    /// A special log type that is interpreted as a background task, on machine readable
    /// outputs it acts as a regular log, but on human-readable outputs it will be 
    /// displayed at the end of the current line.
    #[inline]
    pub fn background_log(&mut self, code: &str) -> Log<'_, true> {
        self._log(code)
    }

}

/// A handle to a log line, allows adding more context to the log.
#[derive(Debug)]
pub struct Log<'a, const BG: bool> {
    /// Exclusive access to output.
    output: &'a mut Output,
    /// Internal buffer.
    shared: &'a mut LogShared,
}

impl<const BG: bool> Log<'_, BG> {

    // Reminder:
    // \x1b[s  save current cursor position
    // \x1b[u  restore saved cursor position
    // \x1b[K  clear the whole line

    /// Append an argument for machine-readable output.
    pub fn arg<D: Display>(&mut self, arg: D) -> &mut Self {
        if let OutputMode::TabSeparated {  } = self.output.mode {
            write!(self.shared.line, "\t{arg}").unwrap();
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
                write!(self.shared.line, "\t{arg}").unwrap();
            }
        }
        self
    }

    /// Internal function to flush the line and background buffers (only relevant in 
    /// human-readable mode)
    fn flush_line_background(&mut self, newline: bool) {

        let mut lock = io::stdout().lock();
        
        if self.output.escape_cursor_cap {
            // If supporting cursor escape code, we don't use carriage return but instead
            // we use cursor save/restore position in order to easily support wrapping.
            lock.write_all(b"\x1b[u\x1b[K").unwrap();
        } else {
            lock.write_all(b"\r").unwrap();
        }

        lock.write_all(self.shared.line.as_bytes()).unwrap();
        lock.write_all(self.shared.background.as_bytes()).unwrap();

        if newline {

            self.shared.line.clear();
            self.shared.background.clear();

            lock.write_all(b"\n").unwrap();
            if self.output.escape_cursor_cap {
                lock.write_all(b"\x1b[s").unwrap();
            }

        }

        lock.flush().unwrap();

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

                self.shared.line.clear();
                if !self.output.escape_color_cap || color.is_empty() {
                    write!(self.shared.line, "[{name:^6}] {message}").unwrap();
                } else {
                    write!(self.shared.line, "[{color}{name:^6}\x1b[0m] {message}").unwrap();
                }

                self.flush_line_background(level != LogLevel::Progress);

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
            
            self.shared.background.clear();
            write!(self.shared.background, "{message}").unwrap();

            self.flush_line_background(false);

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
            // Do nothing in human mode because the message is always immediately
            // flushed to stdout, the buffers may not be empty because if we don't
            // add a newline then the buffer is kept for being rewritten on next log.
        } else {
            // Not in human-readable mode, the buffer has not already been flushed.
            let mut lock = io::stdout().lock();
            lock.write_all(self.shared.line.as_bytes()).unwrap();
            lock.write_all(b"\n").unwrap();
            lock.flush().unwrap();
        }

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

/// The output table mode, used to build a table.
#[derive(Debug)]
pub struct TableOutput<'a> {
    /// Exclusive access to output.
    output: &'a mut Output,
    /// Locked stdout.
    writer: StdoutLock<'static>,
    /// Number of columns.
    columns: usize,
    /// The current column being written.
    column: usize,
    /// This buffer contains all rendered cells. For human-readable only.
    buffer: String,
    /// For each cell, ordered by column and then by row, containing the index where the
    /// cell's content ends in the shared buffer. For human-readable only.
    cells: Vec<usize>,
}

impl<'a> TableOutput<'a> {

    /// Fill the next cell on the right. The given content is considered "raw" and 
    /// unformatted, it will be displayed by default, as-is in all output modes. To
    /// format the cell for human-readable output, you can use the returned handle.
    pub fn cell<D: Display>(&mut self, content: D) -> Cell<'_> {

        match self.output.mode {
            OutputMode::Human { .. } => {
                write!(self.buffer, "{content}").unwrap();
                self.cells.push(self.buffer.len());
            }
            OutputMode::TabSeparated {  } => {
                if self.column == 0 {
                    write!(self.writer, "row").unwrap();
                }
                write!(self.writer, "\t{content}").unwrap();
            }
        }

        self.column += 1;
        if self.column == self.columns {
            self.column = 0;
            if let OutputMode::TabSeparated {  } = self.output.mode {
                self.writer.write_all(b"\n").unwrap();
            }
        }

        Cell {
            output: &mut *self.output,
            buffer: &mut self.buffer,
            cells: &mut self.cells,
        }

    }

    /// Force going to the next row, event if not all cells have been written in the
    /// current line.
    pub fn next_row(&mut self) {

        if let OutputMode::TabSeparated {  } = self.output.mode {
            if self.column == 0 {
                write!(self.writer, "row").unwrap();
            }
        }
        
        for _ in self.column..self.columns {
            if let OutputMode::TabSeparated {  } = self.output.mode {
                write!(self.writer, "\t").unwrap();
            }
            self.cells.push(self.buffer.len());
        }

        if let OutputMode::TabSeparated {  } = self.output.mode {
            self.writer.write_all(b"\n").unwrap();
        }

        self.column = 0;

    }

}

impl Drop for TableOutput<'_> {
    fn drop(&mut self) {
        
        if self.column != 0 {
            self.next_row();
        }

        if let OutputMode::Human { .. } = self.output.mode {

            let mut columns_width = vec![0usize; self.columns];

            // Initially compute maximum width of each column.
            let mut column = 0;
            let mut last_idx = 0;
            for idx in self.cells.iter().copied() {

                columns_width[column] = columns_width[column].max(idx - last_idx);
                last_idx = idx;
                
                column += 1;
                if column == self.columns {
                    column = 0;
                }

            }

            // Reset and restart again to print.
            column = 0;
            last_idx = 0;
            for idx in self.cells.iter().copied() {

                if column != 0 {
                    write!(self.writer, " │ ").unwrap();
                }

                let content = &self.buffer[last_idx..idx];
                last_idx = idx;

                let width = columns_width[column];
                write!(self.writer, "{content:width$}").unwrap();

                column += 1;
                if column == self.columns {
                    column = 0;
                    self.writer.write_all(b"\n").unwrap();
                }

            }

        }

    }
}

/// A handle for customizing metadata and human-readable format.
#[derive(Debug)]
pub struct Cell<'a> {
    output: &'a mut Output,
    buffer: &'a mut String,
    cells: &'a mut Vec<usize>,
}

impl Cell<'_> {

    /// Format this cell differently from the raw data, only for human-readable.
    /// Calling this twice will overwrite the first format.
    pub fn format<D: Display>(&mut self, message: D) -> &mut Self {
        
        if let OutputMode::Human { .. } = self.output.mode {
            // We pop the last cell because it can, and should only be this one.
            self.cells.pop().unwrap();
            // Truncate the old cell's content.
            self.buffer.truncate(self.cells.last().copied().unwrap_or(0));
            // Rewrite the content.
            write!(self.buffer, "{message}").unwrap();
            self.cells.push(self.buffer.len());
        }

        self

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

#[derive(Debug)]
pub struct TimeDeltaDisplay(pub TimeDelta);

impl fmt::Display for TimeDeltaDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        
        let years = self.0.num_days() / 365;
        if years > 0 {
            return write!(f, "{years} years ago");
        }
        
        // All of this is really wrong but it gives a good, human-friendly, idea.
        let months = self.0.num_weeks() / 4;
        if months > 0 {
            return write!(f, "{months} months ago");
        }
        
        let weeks = self.0.num_weeks();
        if weeks > 0 {
            return write!(f, "{weeks} weeks ago");
        }

        let days = self.0.num_days();
        if days > 0 {
            return write!(f, "{days} days ago");
        }

        let hours = self.0.num_hours();
        if hours > 0 {
            return write!(f, "{hours} hours ago");
        }

        let minutes = self.0.num_minutes();
        write!(f, "{minutes} minutes ago")

    }
}
