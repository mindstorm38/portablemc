//! Command line output formats.

use core::fmt;


/// A generic output trait for the command line interface.
pub trait Output {

    fn line(&mut self, message: fmt::Arguments) -> &mut Self;

    fn suffix(&mut self, message: fmt::Arguments) -> &mut Self;

    fn state(&mut self, state: &str, message: fmt::Arguments) -> &mut Self;

    fn newline(&mut self) -> &mut Self;

}
