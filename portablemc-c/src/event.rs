//! Utilities for event and handlers.

use crate::raw;


/// An internal structure to use a pmc_handler type as a regular handler.
pub(crate) struct AdapterHandler(pub raw::pmc_handler);

impl AdapterHandler {

    pub fn forward(&mut self, tag: raw::pmc_event_tag, data: impl Into<raw::pmc_event_data>) {
        if let Some(handler) = self.0 {
            // SAFETY: The handler, when passed to pmc_*_install ensures that it will
            // not leak the event outside and not free it.
            let mut event = raw::pmc_event { tag, data: data.into() };
            unsafe { handler(&mut event) }
        }
    }

}
