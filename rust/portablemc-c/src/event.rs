//! Utilities for event and handlers.

use std::ptr::NonNull;

use crate::alloc::extern_box;
use crate::raw;


/// Allocate an extern event.
pub fn extern_event<D, O>(tag: raw::pmc_event_tag, data: D, owned: O) -> NonNull<raw::pmc_event>
where
    D: Into<raw::pmc_event_data>,
{

    #[repr(C)]
    struct ExternEvent<O> {
        inner: raw::pmc_event,
        owned: O,
    }

    extern_box(ExternEvent {
        inner: raw::pmc_event {
            tag,
            data: data.into(),
        },
        owned,
    }).cast::<raw::pmc_event>()

}
