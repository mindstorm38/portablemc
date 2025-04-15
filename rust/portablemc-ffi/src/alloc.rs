//! Memory allocation management through the FFI boundaries, allowing a unified free 
//! function for every object.

use std::alloc::{self, Layout, handle_alloc_error};
use std::fmt::{Arguments, Write};
use std::ffi::{c_char, c_void};
use std::mem::offset_of;
use std::cell::RefCell;
use std::ptr;


/// Internal type alias for the drop function pointer.
type DropFn<T> = unsafe fn(value_ptr: *mut T);

/// A generic C structure for the extern box.
#[repr(C)]
struct ExternBox<T: 'static> {
    /// The number of values ('value' is also counted).
    len: usize,
    /// The drop function, which is also responsible for deallocating the whole structure,
    /// we only give it the pointer to the value, so depending on the box type it can do
    /// different things.
    /// 
    /// Note that this field should not be access directly, because its actual offset 
    /// might be different depending on the alignment and the fact that we make sure that
    /// it gets placed JUST BEFORE the value. Read [`extern_box_layout`].
    drop: DropFn<T>,
    /// The actual value being stored end pointed to in the FFI.
    value: T,
}

/// Internal function that compute the layout for allocating a `ExternBox<T>` with `len`
/// values.
fn extern_box_layout<T: 'static>(len: usize) -> Layout {

    // We start by allocating the real extern box layout, which may add a padding between
    // the drop fn pointer and the value, to ensure that both are properly padded.
    // Alignment will only be used if 'align_of(value) > align_of(drop)', we might 
    // get a padding (it also depends on the extension value) that is a multiple
    // of 'align_of(drop)' and so we can also move that drop fn pointer at the end of
    // that padding just before the value.
    //
    // Using 'Type = (size, align)' notation for types, and 'field(size)' for fields.
    //
    // With DropFn<T> = (8, 8), T = (N, 16), usize = (8, 8)
    //   -> ExternBox<T, ()> { size(8), drop(8), _(16), value(N) }
    //   => In this case we move the drop fn into the padding.
    //
    // With DropFn<T> = (8, 4), T = (N, 8), usize = (4, 4)
    //   -> ExternBox<T, E> { size(4), drop(8), _(4), value(N) }
    //   => In this case we move the drop by 4 bytes, it will still be aligned.
    //
    // With DropFn<T> = (8, 8), T = (N, 32), E = (4, 4)
    //   -> ExternBox<T, E> { size(4), _(4), drop(8), _(16), value(N) }
    //   => We still have the space to put drop at the end of the padding!
    let layout = Layout::new::<ExternBox<T>>();

    // The ExternBox<T> type only contains one value, we need to adjust for "len" values.
    let size = offset_of!(ExternBox<T>, value) + len * size_of::<T>();

    // SAFETY: Align is a power-of-two because it come from another layout.
    unsafe { Layout::from_size_align_unchecked(size, layout.align()).pad_to_align() }

}

/// The drop impl for the extern box.
unsafe fn extern_box_drop<T: 'static>(value_ptr: *mut T) {

    /// This guard is internally used to ensure that, despite any panic, the 
    /// allocation will be freed!
    struct DeallocGuard<T: 'static>(*mut ExternBox<T>);
    impl<T: 'static> Drop for DeallocGuard<T> {
        fn drop(&mut self) {
            // SAFETY: We access fields safely using offset_of!.
            unsafe {

                let len = self.0.byte_add(offset_of!(ExternBox<T>, len))
                    .cast::<usize>()
                    .read();

                // We can reconstruct the layout because we have the length.
                let layout = extern_box_layout::<T>(len);
                alloc::dealloc(self.0.cast(), layout);

            }
        }
    }

    // SAFETY: We know that this points to 'ExternBox<T>.value' and we access the
    // fields safely using offset_of!.
    unsafe { 
        
        let ptr = value_ptr.byte_sub(offset_of!(ExternBox<T>, value))
            .cast::<ExternBox<T>>();

        let len = ptr.byte_add(offset_of!(ExternBox<T>, len))
            .cast::<usize>()
            .read();

        let values = std::ptr::slice_from_raw_parts_mut(value_ptr, len);

        // Drop all values, guarded to ensure deallocation.
        let guard = DeallocGuard(ptr);
        values.drop_in_place();
        drop(guard);

    }

}

/// Allocate a raw extern box, returning the pointer to uninitialized value(s). 
/// The number of values to put in the allocation must be given by 'len'.
#[inline]
pub fn extern_box_raw<T: 'static>(len: usize) -> *mut T {

    let layout = extern_box_layout::<T>(len);

    // SAFETY: Size can't be one, because we at least have the drop fn pointer.
    let ptr = unsafe { alloc::alloc(layout).cast::<ExternBox<T>>() };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }

    // SAFETY: We point to the different fields safely using offset_of!, read the comment
    // about layout in 'extern_box_layout' to understand that writing the drop fn pointer
    // just before the value is always valid.
    unsafe { 
        
        ptr.byte_add(offset_of!(ExternBox<T>, len))
            .cast::<usize>()
            .write(len);

        ptr.byte_add(offset_of!(ExternBox<T>, value))
            .byte_sub(size_of::<DropFn<T>>())
            .cast::<DropFn<T>>()
            .write(extern_box_drop::<T>);

        ptr.byte_add(offset_of!(ExternBox<T>, value)).cast::<T>()

    }

}

/// Allocate the given object in a special box that also embed the drop function.
#[inline]
pub fn extern_box<T: 'static>(value: T) -> *mut T {
    // SAFETY: The function has reserved enough space to write one value.
    let ptr = extern_box_raw::<T>(1);
    unsafe { ptr.write(value); }
    ptr
}

/// Allocate the given slice of object in a special box that also embed the drop function.
#[inline]
pub fn extern_box_slice<T: Copy + 'static>(slice: &[T]) -> *mut T {
    // SAFETY: The function has reserved enough space to write all values.
    let ptr = extern_box_raw::<T>(slice.len());
    unsafe { ptr.copy_from_nonoverlapping(slice.as_ptr(), slice.len());}
    ptr
}

/// Allocate a C-string from some bytes slice representing a UTF-8 string that may contain
/// a nul byte, any nul byte will truncate early the cstr, the rest will be ignored.
#[inline]
pub fn extern_box_cstr_from_str<S: AsRef<str>>(s: S) -> *mut c_char {
    
    // We immediately treat the input string as bytes, because this is what they are
    // in C, so if there are interior nul bytes, we truncate them.
    let s = s.as_ref();
    let bytes = s.as_bytes();
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

    // Add 1 for the terminating nul.
    let ptr = extern_box_raw::<u8>(len + 1);

    // SAFETY: The function has reserved enough space to write the string with nul.
    unsafe {
        ptr.copy_from_nonoverlapping(bytes.as_ptr(), len);
        ptr.byte_add(len).write(0);
    }

    ptr.cast()

}

/// Allocate a C-string from the string bytes that are formatted with the given args.
#[inline]
pub fn extern_box_cstr_from_fmt(args: Arguments<'_>) -> *mut c_char {

    thread_local! {
        // We use this thread local to
        static BUF: RefCell<String> = RefCell::new(String::new());
    }

    BUF.with_borrow_mut(|buf| {
        // When borrowing, we expect the string to be empty!
        if let Ok(_) = buf.write_fmt(args) {
            let ptr = extern_box_cstr_from_str(buf.as_str());
            buf.clear();
            ptr
        } else {
            ptr::null_mut()
        }
    })
    
}

// =======
// Bindings
// =======

#[no_mangle]
pub extern "C" fn pmc_free(value_ptr: *mut c_void) {

    // Ignore null pointers, this can be used to simplify some code.
    if value_ptr.is_null() {
        return;
    }

    unsafe {
        
        // SAFETY: Read the documentation of 'extern_box_layout' to understand the layout 
        // and the reason for why the drop fn pointer is placed just before the value.
        let drop = value_ptr
            .byte_sub(size_of::<DropFn<c_void>>())
            .cast::<DropFn<c_void>>()
            .read();

        // This drop function, as defined in 'extern_box_drop'.
        drop(value_ptr);

    }

}
