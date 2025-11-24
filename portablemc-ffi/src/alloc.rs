//! Memory allocation management through the FFI boundaries, allowing a unified free 
//! function for every object.

use std::alloc::{self, Layout, handle_alloc_error};
use std::ffi::{c_char, c_void};
use std::fmt::{self, Write};
use std::mem::offset_of;
use std::cell::RefCell;
use std::ptr::NonNull;

use crate::cstr;


/// Internal type alias for the drop function pointer.
type DropFn<T> = unsafe fn(value_ptr: NonNull<T>);

/// A generic C structure for the extern box.
/// 
/// This type should never be instantiated, nor read/write as-is.
#[repr(C)]
struct ExternArray<T> {
    /// The number of values ('value' is also counted).
    len: usize,
    /// The drop function, which is also responsible for deallocating the whole structure,
    /// we only give it the pointer to the value, so depending on the box type it can do
    /// different things.
    /// 
    /// Note that this field should not be accessed directly, because its actual offset 
    /// might be different depending on the alignment and the fact that we make sure that
    /// it gets placed JUST BEFORE the value. Read [`extern_box_layout`].
    drop: DropFn<T>,
    /// The actual value being stored end pointed to in the FFI.
    value: T,
}

/// Internal function that compute the layout for allocating a `ExternBox<T>` with `len`
/// values.
fn extern_array_layout<T>(len: usize) -> Layout {

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
    // With DropFn<T> = (8, 8), T = (N, 32), usize = (4, 4)
    //   -> ExternBox<T, E> { size(4), _(4), drop(8), _(16), value(N) }
    //   => We still have the space to put drop at the end of the padding!
    let layout = Layout::new::<ExternArray<T>>();

    // The ExternBox<T> type only contains one value, we need to adjust for "len" values.
    let size = offset_of!(ExternArray<T>, value) + len * size_of::<T>();

    // SAFETY: Align is a power-of-two because it come from another layout.
    unsafe { Layout::from_size_align_unchecked(size, layout.align()).pad_to_align() }

}

/// Internal function to dealloc the given extern box. This DOES NOT drop the value.
/// 
/// SAFETY: The given extern box pointer should be pointing to an initialized and valid
/// extern box of that exact type!
unsafe fn extern_array_free_unchecked<T>(ptr: NonNull<ExternArray<T>>) {
    unsafe {

        let len = ptr
            .byte_add(offset_of!(ExternArray<T>, len))
            .cast::<usize>()
            .read();

        // We can reconstruct the layout because we have the length.
        let layout = extern_array_layout::<T>(len);
        alloc::dealloc(ptr.as_ptr().cast(), layout);

    }
}

/// Drop the given extern-boxed value and then free the full allocation.
/// 
/// SAFETY: The given extern box pointer should be pointing to an initialized and valid
/// extern box's value of that exact type!
pub unsafe fn extern_box_drop_unchecked<T>(value_ptr: NonNull<T>) {

    /// This guard is internally used to ensure that, despite any panic, the 
    /// allocation will be freed!
    struct FreeGuard<T>(NonNull<ExternArray<T>>);
    impl<T> Drop for FreeGuard<T> {
        fn drop(&mut self) {
            // SAFETY: The SAFETY conditions of the super method applies here.
            unsafe { 
                extern_array_free_unchecked(self.0);
            }
        }
    }

    // SAFETY: We know that this points to 'ExternBox<T>.value' and we access the
    // fields safely using offset_of!.
    unsafe { 

        let ptr = value_ptr
            .byte_sub(offset_of!(ExternArray<T>, value))
            .cast::<ExternArray<T>>();

        let len = ptr
            .byte_add(offset_of!(ExternArray<T>, len))
            .cast::<usize>()
            .read();

        let guard = FreeGuard(ptr);
        std::ptr::slice_from_raw_parts_mut(value_ptr.as_ptr(), len).drop_in_place();
        drop(guard);

    }

}

/// Allocate a raw extern box, returning the pointer to uninitialized value(s). 
/// The number of values to put in the allocation must be given by 'len'.
#[inline]
fn extern_array_raw<T>(len: usize) -> NonNull<T> {

    let layout = extern_array_layout::<T>(len);

    // SAFETY: Size can't be one, because we at least have the drop fn pointer.
    let ptr = unsafe { alloc::alloc(layout).cast::<ExternArray<T>>() };
    let Some(ptr) = NonNull::new(ptr) else {
        handle_alloc_error(layout);
    };

    // SAFETY: We point to the different fields safely using offset_of!, read the comment
    // about layout in 'extern_box_layout' to understand that writing the drop fn pointer
    // just before the value is always valid.
    unsafe { 
        
        ptr.byte_add(offset_of!(ExternArray<T>, len))
            .cast::<usize>()
            .write(len);

        ptr.byte_add(offset_of!(ExternArray<T>, value))
            .byte_sub(size_of::<DropFn<T>>())
            .cast::<DropFn<T>>()
            .write(extern_box_drop_unchecked::<T>);

        ptr.byte_add(offset_of!(ExternArray<T>, value)).cast::<T>()

    }

}

/// Allocate the given object in a special box that also embed the drop function.
#[inline]
pub fn extern_box<T>(value: T) -> NonNull<T> {
    // SAFETY: The function has reserved enough space to write one value.
    let ptr = extern_array_raw::<T>(1);
    unsafe { ptr.write(value); }
    ptr
}

/// Allocate the given slice of object in a special box that also embed the drop function.
#[inline]
pub fn extern_array_from_slice<T: Copy>(slice: &[T]) -> NonNull<T> {
    // SAFETY: The function has reserved enough space to write all values.
    let ptr = extern_array_raw::<T>(slice.len());
    unsafe { ptr.as_ptr().copy_from_nonoverlapping(slice.as_ptr(), slice.len());}
    ptr
}

/// Allocate a C-string from some bytes slice representing a UTF-8 string that may contain
/// a nul byte, any nul byte will truncate early the cstr, the rest will be ignored.
pub fn extern_cstr_from_str(s: &str) -> NonNull<c_char> {
    
    // Immediately safely find the CStr from the input UTF-8 string.
    let cstr = cstr::from_ref(s);

    // Add 1 for the terminating nul.
    let ptr = extern_array_raw::<c_char>(cstr.len() + 1);

    // SAFETY: The function has reserved enough space to write the string with nul.
    unsafe {
        ptr.as_ptr().copy_from_nonoverlapping(cstr.as_ptr(), cstr.len());
        ptr.byte_add(cstr.len()).write(0);
    }

    ptr

}

/// Allocate a C-string from the string bytes that are formatted with the given args.
pub fn extern_cstr_from_fmt(args: fmt::Arguments<'_>) -> Result<NonNull<c_char>, fmt::Error> {

    thread_local! {
        // We use this thread local to
        static BUF: RefCell<String> = RefCell::new(String::new());
    }

    BUF.with_borrow_mut(|buf| {
        // When borrowing, we expect the string to be empty!
        buf.write_fmt(args)?;
        let ptr = extern_cstr_from_str(&buf);
        buf.clear();
        Ok(ptr)
    })
    
}

/// Free the extern box pointing to the given value and return the given value.
/// 
/// SAFETY: You must ensure that the value does point to an extern-boxed value that has
/// no yet been freed nor taken, exactly of the given type.
#[inline]
pub unsafe fn extern_box_take<T>(value_ptr: NonNull<T>) -> T {
    
    // SAFETY: This function pre-condition ensure correctness of the reads.
    unsafe {

        let ptr = value_ptr
            .byte_sub(offset_of!(ExternArray<T>, value))
            .cast::<ExternArray<T>>();

        let len = ptr
            .byte_add(offset_of!(ExternArray<T>, len))
            .cast::<usize>()
            .read();

        if len != 1 {
            panic!("given extern array must have a single value to be interpreted as a box");
        }

        // Start by reading the value, now the value at that position should never be 
        // read again, so we free that function.
        let read = value_ptr.read();

        // We're juste freeing the memory, not dropping the value, because we should not.
        extern_array_free_unchecked(ptr);

        read

    }

}

// =======
// Binding
// =======

/// SAFETY: You must ensure that the value does point to an extern-boxed value that has
/// no yet been freed. The pointer may be null, in which case nothing happens.
#[no_mangle]
pub unsafe extern "C" fn pmc_free(value_ptr: *mut c_void) {

    // Ignore null pointers, this can be used to simplify some code.
    let Some(value_ptr) = NonNull::new(value_ptr) else {
        return;
    };

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

#[cfg(test)]
mod tests {

    use std::fmt::Debug;
    use super::*;

    
    #[repr(align(16))]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    struct Align16([u8; 16]);

    #[repr(align(32))]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    struct Align32([u8; 32]);

    #[test]
    fn layout() {

        fn for_type<T: 'static>() {
            assert_eq!(Layout::new::<DropFn<T>>(), Layout::new::<fn()>());
            assert_eq!(extern_array_layout::<T>(0).size(), offset_of!(ExternArray<T>, value));
            assert_eq!(extern_array_layout::<T>(1).size(), size_of::<ExternArray<T>>());
            assert_eq!(extern_array_layout::<T>(9).size(), size_of::<ExternArray<T>>() + size_of::<T>() * 8);
        }

        for_type::<u8>();
        for_type::<u16>();
        for_type::<u32>();
        for_type::<u64>();
        for_type::<Align16>();
        for_type::<Align32>();

    }

    #[test]
    fn structure() {

        fn for_value<T: Copy + Eq + Debug + 'static>(value: T) {
            unsafe {

                let ptr = extern_box(value);
                assert_eq!(ptr.read(), value, "incoherent read value");

                let drop = ptr
                    .byte_sub(size_of::<DropFn<T>>())
                    .cast::<DropFn<T>>();
                assert!(drop.is_aligned(), "unaligned drop function");

                extern_box_drop_unchecked(ptr);

            }
        }

        for_value(0x12u8);
        for_value(0x1234u16);
        for_value(0x12345678u32);
        for_value(0x123456789ABCDEF0u64);
        for_value(Align16::default());
        for_value(Align32::default());

    }

    #[test]
    fn cstr() {

        /// NOTE: c_len don't count nul.
        fn for_str(s: &str, c_len: usize) {
            let cstr = extern_cstr_from_str(s);
            let cstr_slice = unsafe { std::slice::from_raw_parts(cstr.as_ptr().cast::<u8>(), c_len + 1) };
            assert_eq!(&cstr_slice[..c_len], &s.as_bytes()[..c_len], "incoherent cstr");
            assert_eq!(cstr_slice[c_len], 0, "missing nul");
            unsafe { extern_box_drop_unchecked(cstr); }
        }

        for_str("Hello world!", 12);
        for_str("Hello world!\0", 12);
        for_str("Hello world!\0rest", 12);

    }

}
