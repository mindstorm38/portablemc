//! Various utilities to ease outputting human or machine readable text.

use std::fmt::{self, Display, Write as _};
use std::io::{IsTerminal, Write};
use std::{env, io};


/// An abstraction for outputting to any format on stdout, the goal is to provide an
/// interface for outputting at the same time both human readable and machine outputs.
/// 
/// The different supported output formats are basically split in two kins: machine 
/// readable and human readable. All functions in this abstraction are machine-oriented,
/// that means that by default the human representation will be the machine one (or no 
/// representation at all), but the different handles returned can be used to customize
/// or add human representation.
#[derive(Debug, Clone)]
pub struct Output {
    /// Mode-specific data.
    mode: OutputMode,
    /// Are cursor escape code supported on stdout.
    escape_cursor_cap: bool,
    /// Are color escape code supported on stdout.
    escape_color_cap: bool,
}

#[derive(Debug, Clone)]
enum OutputMode {
    Human(OutputHuman),
    TabSep(OutputTabSep),
}

#[derive(Debug, Clone)]
struct OutputHuman {
    /// All log lines below this level are discarded.
    log_level: LogLevel,
    /// Set to true when the current line the cursor should be on is a new one (empty).
    log_newline: bool,
    /// Set to true when the previous log line has been successfully displayed (regarding
    /// the log level).
    log_last_level: LogLevel,
    /// Line buffer that will be printed for each line.
    log_line: String,
    /// Storing the rendered background log.
    log_background: String,
    /// This buffer contains all rendered cells for human-readable.
    table_buffer: String,
    /// For each cell, ordered by column and then by row, containing the index where the
    /// cell's content ends in the shared buffer.
    table_cells: Vec<usize>,
    /// Stores for each separator the index of the row it's placed before.
    table_separators: Vec<usize>,
}

#[derive(Debug, Clone)]
struct OutputTabSep {
    /// Line buffer that will be printed when the log is dropped.
    buffer: String,
}

impl Output {

    pub fn human(log_level: LogLevel) -> Self {
        Self::new(OutputMode::Human(OutputHuman {
            log_level,
            log_newline: true,
            log_last_level: LogLevel::Info,
            log_line: String::new(),
            log_background: String::new(),
            table_buffer: String::new(),
            table_cells: Vec::new(),
            table_separators: Vec::new(),
        }))
    }

    pub fn tab_separated() -> Self {
        Self::new(OutputMode::TabSep(OutputTabSep {
            buffer: String::new(),
        }))
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

    /// Return true if the output mode is human-readable.
    #[inline]
    pub fn is_human(&self) -> bool {
        matches!(self.mode, OutputMode::Human(_))
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
    pub fn log_background(&mut self, code: impl Display) -> Log<'_, true> {
        self._log(code)
    }

    /// Internal implementation detail used to be generic over being background.
    #[inline]
    fn _log<const BG: bool>(&mut self, code: impl Display) -> Log<'_, BG> {

        match &mut self.mode {
            OutputMode::Human(_mode) => {
                // // Save the cursor at the beginning of the new line.
                // if self.escape_cursor_cap && mode.log_newline {
                //     print!("\x1b[s");
                // }
            }
            OutputMode::TabSep(mode) => {
                debug_assert!(mode.buffer.is_empty());
                write!(mode.buffer, "{code}").unwrap();
            }
        }

        Log {
            output: self,
        }

    }

    /// Enter table mode, this is exclusive with other modes.
    pub fn table(&mut self, columns: usize) -> TableOutput<'_> {
        
        assert_ne!(columns, 0);

        match &mut self.mode {
            OutputMode::Human(mode) => {
                
                if !mode.log_newline {
                    println!();
                    mode.log_newline = true;
                    mode.log_line.clear();
                    mode.log_background.clear();
                }

                debug_assert!(mode.table_buffer.is_empty());
                debug_assert!(mode.table_cells.is_empty());
                debug_assert!(mode.table_separators.is_empty());

            }
            OutputMode::TabSep(mode) => {
                debug_assert!(mode.buffer.is_empty());
            }
        };

        TableOutput {
            output: self,
            columns,
        }

    }

}

/// A handle to a log line, allows adding more context to the log.
#[derive(Debug)]
pub struct Log<'a, const BG: bool> {
    /// Exclusive access to output.
    output: &'a mut Output,
}

impl<const BG: bool> Log<'_, BG> {

    // Reminder:
    // \x1b[s  save current cursor position
    // \x1b[u  restore saved cursor position
    // \x1b[K  clear the whole line

    /// Append an argument for machine-readable output.
    pub fn arg(&mut self, arg: impl Display) -> &mut Self {
        if let OutputMode::TabSep(mode) = &mut self.output.mode {
            write!(mode.buffer, "\t{}", EscapeTabSeparatedValue(arg)).unwrap();
        }
        self
    }

    /// Append many arguments for machine-readable output.
    pub fn args<D, I>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = D>,
        D: Display,
    {
        if let OutputMode::TabSep(mode) = &mut self.output.mode {
            for arg in args {
                write!(mode.buffer, "\t{}", EscapeTabSeparatedValue(arg)).unwrap();
            }
        }
        self
    }

    /// Internal function to flush the line and background buffers, should only be
    /// called in human readable mode.
    fn flush_human_line(&mut self, newline: bool) {

        let OutputMode::Human(mode) = &mut self.output.mode else { panic!() };

        let mut lock = io::stdout().lock();
        
        if self.output.escape_cursor_cap {
            // If supporting cursor escape code, we don't use carriage return but instead
            // we use cursor save/restore position in order to easily support wrapping.
            if mode.log_newline {
                // If the line is currently empty, save the cursor position!
                lock.write_all(b"\x1b[s").unwrap();
            } else {
                // If the line is not empty, restore saved cursor position and clear line.
                lock.write_all(b"\x1b[u\x1b[K").unwrap();
            }
        } else {
            lock.write_all(b"\r").unwrap();
        }

        lock.write_all(mode.log_line.as_bytes()).unwrap();
        if !mode.log_line.is_empty() && !mode.log_background.is_empty() {
            lock.write_all(b" -- ").unwrap();
        }
        lock.write_all(mode.log_background.as_bytes()).unwrap();

        if newline {

            mode.log_line.clear();
            mode.log_background.clear();
            mode.log_newline = true;

            lock.write_all(b"\n").unwrap();

        } else {
            mode.log_newline = false;
        }

        lock.flush().unwrap();

    }

}

impl Log<'_, false> {

    /// Only relevant for human-readable messages, it forces a newline to be added if the
    /// current line's level is "pending" without overwriting is 
    pub fn newline(&mut self) -> &mut Self {
        if let OutputMode::Human(mode) = &mut self.output.mode {
            if !mode.log_newline {
                println!();
                mode.log_line.clear();
                mode.log_background.clear();
                mode.log_newline = true;
            }
        }
        self
    }

    /// Append a human-readable message to this log with an associated level, level is
    /// only relevant here because machine-readable outputs are always verbose.
    pub fn line(&mut self, level: LogLevel, message: impl Display) -> &mut Self {
        if let OutputMode::Human(mode) = &mut self.output.mode {
            let last_level = std::mem::replace(&mut mode.log_last_level, level);
            if level >= mode.log_level {

                let (name, color) = match level {
                    LogLevel::Info => ("INFO", "\x1b[34m"),
                    LogLevel::Pending => ("..", ""),
                    LogLevel::Success => ("OK", "\x1b[92m"),
                    LogLevel::Warn => ("WARN", "\x1b[33m"),
                    LogLevel::Error => ("ERRO", "\x1b[31m"),
                    LogLevel::Additional => {
                        mode.log_last_level = last_level; // Cancel the change.
                        if last_level < mode.log_level {
                            return self;
                        } else {
                            ("a", "")
                        }
                    }
                    LogLevel::Raw => ("r", ""), 
                    LogLevel::RawWarn => ("r", "\x1b[33m"), 
                    LogLevel::RawError => ("r", "\x1b[31m"), 
                    LogLevel::RawFatal => ("r", "\x1b[1;31m"), 
                };

                mode.log_line.clear();
                if name == "a" {
                    write!(mode.log_line, "         {message}").unwrap();
                } else if name == "r" {
                    if !self.output.escape_color_cap || color.is_empty() {
                        write!(mode.log_line, "{message}").unwrap();
                    } else {
                        write!(mode.log_line, "{color}{message}\x1b[0m").unwrap();
                    }
                } else {
                    if !self.output.escape_color_cap || color.is_empty() {
                        write!(mode.log_line, "[{name:^6}] {message}").unwrap();
                    } else {
                        write!(mode.log_line, "[{color}{name:^6}\x1b[0m] {message}").unwrap();
                    }
                }

                self.flush_human_line(level != LogLevel::Pending);

            }
        }
        self
    }

    #[inline]
    pub fn info(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Info, message)
    }

    #[inline]
    pub fn pending(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Pending, message)
    }

    #[inline]
    pub fn success(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Success, message)
    }

    #[inline]
    pub fn warning(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Warn, message)
    }

    #[inline]
    pub fn error(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Error, message)
    }

    #[inline]
    pub fn additional(&mut self, message: impl Display) -> &mut Self {
        self.line(LogLevel::Additional, message)
    }

}

impl Log<'_, true> {
    
    /// Set the human-readable message of this background log. Note that this will 
    /// overwrite any background message currently written on the current log line.
    pub fn message(&mut self, message: impl Display) -> &mut Self {
        if let OutputMode::Human(mode) = &mut self.output.mode {
            
            mode.log_background.clear();
            write!(mode.log_background, "{message}").unwrap();

            self.flush_human_line(false);

        }
        self
    }

}

impl<const BACKGROUND: bool> Drop for Log<'_, BACKGROUND> {
    fn drop(&mut self) {
        match &mut self.output.mode {
            OutputMode::Human(_) => {
                // Do nothing in human mode because the message is always immediately
                // flushed to stdout, the buffers may not be empty because if we don't
                // add a newline then the buffer is kept for being rewritten on next log.
            }
            OutputMode::TabSep(mode) => {
                // Not in human-readable mode, the buffer has not already been flushed.
                let mut lock = io::stdout().lock();
                lock.write_all(mode.buffer.as_bytes()).unwrap();
                mode.buffer.clear();
                lock.write_all(b"\n").unwrap();
                lock.flush().unwrap();
            }
        }

    }
}

/// Level for a human-readable log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// This log is something indicative, discarded when not in verbose mode.
    Info = 0,
    /// This log indicate something is in progress and the definitive state is unknown.
    /// If the next log is another pending or a success, it will overwrite this log, if
    /// not the next log will be printed on the next line.
    Pending = 1,
    /// This log indicate a success.
    Success = 2,
    /// This log is a warning.
    Warn = 3,
    /// This log is an error.
    Error = 4,
    /// An additional log, related to the previous one. This will only be displayed if
    /// the previous log has been displayed, its level will be the same as the previous
    /// log (discarded by the level or not), but without the header.
    Additional = 100,
    /// A raw log line to be displayed without the header.
    Raw = 200,
    /// Same as [`Self::Raw`] but warning-colored if supported by the terminal.
    RawWarn = 201,
    /// Same as [`Self::Raw`] but error-colored if supported by the terminal.
    RawError = 202,
    /// Same as [`Self::Raw`] but fatal-colored if supported by the terminal.
    RawFatal = 203,
}

/// The output table mode, used to build a table.
#[derive(Debug)]
pub struct TableOutput<'a> {
    /// Exclusive access to output.
    output: &'a mut Output,
    /// Number of columns.
    columns: usize,
}

impl<'a> TableOutput<'a> {

    /// Create a new row, returning a handle for writing its cells.
    pub fn row(&mut self) -> Row<'_> {

        match &mut self.output.mode {
            OutputMode::Human(mode) => {
                // Just to ensure that cells count is padded when 'Row' is dropped.
                debug_assert!(mode.table_cells.len().checked_rem(self.columns).unwrap_or(0) == 0);
            }
            OutputMode::TabSep(mode) => {
                debug_assert!(mode.buffer.is_empty());
                write!(mode.buffer, "row").unwrap();
            }
        }

        Row {
            output: &mut self.output,
            column_remaining: self.columns,
        }

    }

    /// Insert a separator, this is used for human-readable format but also for
    /// machine-readable formats in order to separate different sections of data
    /// (they still have the same number of columns), such as the header from the
    /// rest of the data.
    pub fn sep(&mut self) {
        
        match &mut self.output.mode {
            OutputMode::Human(mode) => {
                // Just to ensure that cells count is padded when 'Row' is dropped.
                debug_assert!(mode.table_cells.len().checked_rem(self.columns).unwrap_or(0) == 0);
                mode.table_separators.push(mode.table_cells.len() / self.columns);
            }
            OutputMode::TabSep(mode) => {
                debug_assert!(mode.buffer.is_empty());
                println!("sep");
            }
        }

    }

}

impl Drop for TableOutput<'_> {
    fn drop(&mut self) {
        
        if let OutputMode::Human(mode) = &mut self.output.mode {

            let mut columns_width = vec![0usize; self.columns];

            // Initially compute maximum width of each column.
            let mut column = 0usize;
            let mut last_idx = 0usize;
            for idx in mode.table_cells.iter().copied() {

                columns_width[column] = columns_width[column].max(idx - last_idx);
                last_idx = idx;
                
                column += 1;
                if column == self.columns {
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

            let mut separators: _ = mode.table_separators.iter().copied().peekable();
            let mut writer = io::stdout().lock();

            // Write top segment.
            write!(writer, "┌─").unwrap();
            write_separator(&mut writer, "─┬─");
            write!(writer, "─┐\n").unwrap();

            // Reset and restart again to print.
            let mut row = 0usize;
            let mut column = 0usize;
            let mut last_idx = 0usize;
            for idx in mode.table_cells.iter().copied() {

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

                let content = &mode.table_buffer[last_idx..idx];
                last_idx = idx;

                let width = columns_width[column];
                write!(writer, "{content:width$}").unwrap();

                column += 1;
                if column == self.columns {
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

            mode.table_buffer.clear();
            mode.table_cells.clear();
            mode.table_separators.clear();

        }

    }
}

/// A handle for constructing a table row.
#[derive(Debug)]
pub struct Row<'a> {
    output: &'a mut Output,
    column_remaining: usize,
}

impl Row<'_> {

    /// Insert a new cell to that row with the given machine-readable content, to add
    /// a formatted human-readable string, use the returned cell handle. By default,
    /// the human representation is the given content.
    #[track_caller]
    pub fn cell(&mut self, content: impl Display) -> Cell<'_> {
        
        if self.column_remaining == 0 {
            panic!("no remaining column");
        }

        match &mut self.output.mode {
            OutputMode::Human(mode) => {
                write!(mode.table_buffer, "{content}").unwrap();
                mode.table_cells.push(mode.table_buffer.len());
            }
            OutputMode::TabSep(mode) => {
                write!(mode.buffer, "\t{content}").unwrap();
            }
        }

        self.column_remaining -= 1;

        Cell {
            output: &mut self.output,
        }

    }

}

impl Drop for Row<'_> {
    fn drop(&mut self) {
        match &mut self.output.mode {
            OutputMode::Human(mode) => {
                for _ in 0..self.column_remaining {
                    mode.table_cells.push(mode.table_buffer.len());
                }
            }
            OutputMode::TabSep(mode) => {
                for _ in 0..self.column_remaining {
                    mode.buffer.push('\t');
                }
                println!("{}", mode.buffer);
                mode.buffer.clear();
            }
        }
    }
}

/// A handle for customizing metadata and human-readable format.
#[derive(Debug)]
pub struct Cell<'a> {
    output: &'a mut Output,
}

impl Cell<'_> {

    /// Format this cell differently from the raw data, only for human-readable.
    /// Calling this twice will overwrite the first format.
    pub fn format<D: Display>(&mut self, message: D) -> &mut Self {
        
        if let OutputMode::Human(mode) = &mut self.output.mode {
            // We pop the last cell because it can, and should only be this one.
            mode.table_cells.pop().unwrap();
            // Truncate the old cell's content.
            mode.table_buffer.truncate(mode.table_cells.last().copied().unwrap_or(0));
            // Rewrite the content.
            write!(mode.table_buffer, "{message}").unwrap();
            mode.table_cells.push(mode.table_buffer.len());
        }

        self

    }

}

/// Internal display wrapper to escape any newline character '\n' by a literal escape 
/// "\\n", this is used for tab-separated output to avoid early line return before end
/// of the line.
struct EscapeTabSeparatedValue<T>(T);

impl<T: Display> fmt::Display for EscapeTabSeparatedValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        
        struct Wrapper<'a, 'b> {
            f: &'a mut fmt::Formatter<'b>,
        }

        impl fmt::Write for Wrapper<'_, '_> {

            fn write_str(&mut self, mut s: &str) -> fmt::Result {
                'out: loop {
                    for (i, ch) in s.char_indices() {

                        let repl = match ch {
                            '\n' => "\\n",
                            '\t' => "\\t",
                            _ => continue,
                        };

                        self.f.write_str(&s[..i])?;
                        self.f.write_str(repl)?;
                        s = &s[i + 1..];
                        continue 'out;

                    }
                    break;  // In case no more escapable character...
                }
                self.f.write_str(s)?;
                Ok(())
            }

            fn write_char(&mut self, c: char) -> fmt::Result {
                match c {
                    '\n' => self.f.write_str("\\n"),
                    '\t' => self.f.write_str("\\t"),
                    _ => self.f.write_char(c)
                }
            }

        }

        let mut wrapper = Wrapper { f };
        write!(wrapper, "{}", self.0)

    }
}
