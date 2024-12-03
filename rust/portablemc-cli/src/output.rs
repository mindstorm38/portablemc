//! Various utilities to ease outputting human or machine readable text.

use std::fmt::{self, Display, Write as _};
use std::io::{IsTerminal, Write};
use std::{env, io};

use chrono::TimeDelta;


/// An abstraction for outputting to any format on stdout, the goal is to provide an
/// interface for outputting at the same time both human readable and machine outputs.
/// 
/// The different supported output formats are basically split in two kins: machine 
/// readable and human readable. All functions in this abstraction are machine-oriented,
/// that means that by default the human representation will be the machine one (or no 
/// representation at all), but the different handles returned can be used to customize
/// or a add human representation.
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
            shared: TableShared {
                columns,
                ..TableShared::default()
            },
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

impl LogOutput<'_> {

    /// Internal implementation detail used to be generic over being background.
    #[inline]
    fn _log<const BG: bool>(&mut self, code: impl Display) -> Log<'_, BG> {

        if let OutputMode::TabSeparated {  } = self.output.mode {
            debug_assert!(self.shared.line.is_empty());
            write!(self.shared.line, "{code}").unwrap();
        }

        Log {
            output: &mut self.output,
            shared: &mut self.shared,
        }

    }

    /// Log an information with a simple code referencing it, the given code is the 
    /// machine-readable code, to add human-readable line use the returned handle.
    #[inline]
    pub fn log(&mut self, code: impl Display) -> Log<'_, false> {
        self._log(code)
    }

    /// A special log type that is interpreted as a background task, on machine readable
    /// outputs it acts as a regular log, but on human-readable outputs it will be 
    /// displayed at the end of the current line.
    #[inline]
    pub fn background_log(&mut self, code: impl Display) -> Log<'_, true> {
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
    pub fn arg(&mut self, arg: impl Display) -> &mut Self {
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

        debug_assert!(matches!(self.output.mode, OutputMode::Human { .. }));

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
    pub fn line(&mut self, level: LogLevel, message: impl Display) -> &mut Self {
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
    pub fn info(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Info, message)
    }

    #[inline]
    pub fn progress(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Progress, message)
    }

    #[inline]
    pub fn success(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Success, message)
    }

    #[inline]
    pub fn warning(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Warning, message)
    }

    #[inline]
    pub fn error(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Error, message)
    }

}

impl Log<'_, true> {
    
    /// Set the human-readable message of this background log. Note that this will 
    /// overwrite any background message currently written on the current log line.
    pub fn message(&mut self, message: impl Display) -> &mut Self {
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
    /// Data shared with row and cell handles.
    shared: TableShared,
}

#[derive(Debug, Default)]
struct TableShared {
    /// Number of columns.
    columns: usize,
    /// This buffer contains all rendered cells for human-readable, for machine readable
    /// it's used to store the current rendered row.
    buffer: String,
    /// For each cell, ordered by column and then by row, containing the index where the
    /// cell's content ends in the shared buffer.
    /// For human-readable only.
    cells: Vec<usize>,
    /// Stores for each separator the index of the row it's placed before.
    /// For human-readable only.
    separators: Vec<usize>,
}

impl<'a> TableOutput<'a> {

    /// Create a new row, returning a handle for writing its cells.
    pub fn row(&mut self) -> Row<'_> {

        debug_assert!(self.shared.cells.len().checked_rem(self.shared.columns).unwrap_or(0) == 0);
        
        if let OutputMode::TabSeparated {  } = self.output.mode {
            debug_assert!(self.shared.buffer.is_empty());
            write!(self.shared.buffer, "row").unwrap();
        }

        Row {
            output: &mut self.output,
            shared: &mut self.shared,
            column: 0,
        }

    }

    /// Insert a separator, this is used for human-readable format but also for
    /// machine-readable formats in order to separate different sections of data
    /// (they still have the same number of columns), such as the header from the
    /// rest of the data.
    pub fn sep(&mut self) {
        
        debug_assert!(self.shared.cells.len().checked_rem(self.shared.columns).unwrap_or(0) == 0);
        
        match self.output.mode {
            OutputMode::Human { .. } => {
                let index = self.shared.cells.len().checked_div(self.shared.columns).unwrap_or(0);
                self.shared.separators.push(index);
            }
            OutputMode::TabSeparated {  } => {
                println!("sep");
            }
        }

    }

}

impl Drop for TableOutput<'_> {
    fn drop(&mut self) {
        
        if let OutputMode::Human { .. } = self.output.mode {


            let mut columns_width = vec![0usize; self.shared.columns];

            // Initially compute maximum width of each column.
            let mut column = 0usize;
            let mut last_idx = 0usize;
            for idx in self.shared.cells.iter().copied() {

                columns_width[column] = columns_width[column].max(idx - last_idx);
                last_idx = idx;
                
                column += 1;
                if column == self.shared.columns {
                    column = 0;
                }

            }

            // Small closure just to write a separator.
            let write_separator: _ = |writer: &mut io::StdoutLock<'_>, join: &str| {
                for (col, width) in columns_width.iter().copied().enumerate() {
                    if col != 0 {
                        writer.write_all(join.as_bytes()).unwrap();
                    }
                    write!(writer, "{:─<width$}", "").unwrap();
                }
            };

            let mut separators: _ = self.shared.separators.iter().copied().peekable();
            let mut writer = io::stdout().lock();

            // Write top segment.
            write!(writer, "┌─").unwrap();
            write_separator(&mut writer, "─┬─");
            write!(writer, "─┐\n").unwrap();

            // Reset and restart again to print.
            let mut row = 0usize;
            let mut column = 0usize;
            let mut last_idx = 0usize;
            for idx in self.shared.cells.iter().copied() {

                if column != 0 {
                    write!(writer, " │ ").unwrap();
                } else {

                    if separators.next_if_eq(&row).is_some() {
                        write!(writer, "├─").unwrap();
                        write_separator(&mut writer, "─┼─");
                        write!(writer, "─┤\n").unwrap();
                    }

                    write!(writer, "│ ").unwrap();

                }

                let content = &self.shared.buffer[last_idx..idx];
                last_idx = idx;

                let width = columns_width[column];
                write!(writer, "{content:width$}").unwrap();

                column += 1;
                if column == self.shared.columns {
                    row += 1;
                    column = 0;
                    write!(writer, " │\n").unwrap();
                }

            }

            // It's a really special case that will never happen, add last separator.
            if separators.next_if_eq(&row).is_some() {
                write!(writer, "├─").unwrap();
                write_separator(&mut writer, "─┼─");
                write!(writer, "─┤\n").unwrap();
            }

            // Write bottom segment.
            write!(writer, "└─").unwrap();
            write_separator(&mut writer, "─┴─");
            write!(writer, "─┘\n").unwrap();

        }

    }
}

/// A handle for constructing a table row.
#[derive(Debug)]
pub struct Row<'a> {
    output: &'a mut Output,
    shared: &'a mut TableShared,
    column: usize,
}

impl Row<'_> {

    /// Insert a new cell to that row with the given machine-readable content, to add
    /// a formatted human-readable string, use the returned cell handle.
    #[track_caller]
    pub fn cell(&mut self, content: impl Display) -> Cell<'_> {
        
        if self.column == self.shared.columns {
            panic!("too much cells in this row");
        }

        match self.output.mode {
            OutputMode::Human { .. } => {
                write!(self.shared.buffer, "{content}").unwrap();
                self.shared.cells.push(self.shared.buffer.len());
            }
            OutputMode::TabSeparated {  } => {
                write!(self.shared.buffer, "\t{content}").unwrap();
            }
        }

        self.column += 1;

        Cell {
            output: &mut self.output,
            shared: &mut self.shared,
        }

    }

}

impl Drop for Row<'_> {
    fn drop(&mut self) {

        // Add missing columns' cells.
        match self.output.mode {
            OutputMode::Human { .. } => {
                for _ in self.column..self.shared.columns {
                    self.shared.cells.push(self.shared.buffer.len());
                }
            }
            OutputMode::TabSeparated {  } => {
                for _ in self.column..self.shared.columns {
                    self.shared.buffer.push('\t');
                }
            }
        }

        if let OutputMode::TabSeparated {  } = self.output.mode {
            println!("{}", self.shared.buffer);
            self.shared.buffer.clear();
        }

    }
}

/// A handle for customizing metadata and human-readable format.
#[derive(Debug)]
pub struct Cell<'a> {
    output: &'a mut Output,
    shared: &'a mut TableShared,
}

impl Cell<'_> {

    /// Format this cell differently from the raw data, only for human-readable.
    /// Calling this twice will overwrite the first format.
    pub fn format<D: Display>(&mut self, message: D) -> &mut Self {
        
        if let OutputMode::Human { .. } = self.output.mode {
            // We pop the last cell because it can, and should only be this one.
            self.shared.cells.pop().unwrap();
            // Truncate the old cell's content.
            self.shared.buffer.truncate(self.shared.cells.last().copied().unwrap_or(0));
            // Rewrite the content.
            write!(self.shared.buffer, "{message}").unwrap();
            self.shared.cells.push(self.shared.buffer.len());
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

/// A wrapper that can be used to format a time delta for human-readable format.
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
