//! Various formatting utilities.

use std::fmt;

use chrono::TimeDelta;


/// Common human-readable date format.
pub const DATE_FORMAT: &str = "%a %b %e %T %Y";
/// Common human-readable time format (for logs).
pub const TIME_FORMAT: &str = "%T";

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
        let months = self.0.num_days() / 30;
        if months > 0 {
            return write!(f, "{months} months ago");
        }
        
        let weeks = self.0.num_days() / 7;
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
