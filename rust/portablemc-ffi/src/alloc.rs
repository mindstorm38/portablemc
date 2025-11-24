//! Memory allocation management through the FFI boundaries, allowing a unified free 
//! function for every object.

use std::alloc::{self, Layout, handle_alloc_error};
use std::fmt::{Arguments, Write};
use std::ffi::{c_char, c_void};
use std::mem::offset_of;
use std::cell::RefCell;
use std::ptr;

#[cfg(debug_assertions)]
use std::any::TypeId;


/// Internal type alias for the drop function pointer.
type DropFn<T> = unsafe fn(value_ptr: *mut T);

/// A generic C structure for the extern box.
/// 
/// This type should never be instantiated, nor read/write as-is.
#[repr(C)]
struct ExternBox<T: 'static> {
    /// When debug assertions are enabled, this is used to check, before doing unsafe
    /// stuff, that the interpreted type is correct.
    #[cfg(debug_assertions)]
    type_id: TypeId,
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
    // With DropFn<T> = (8, 8), T = (N, 32), usize = (4, 4)
    //   -> ExternBox<T, E> { size(4), _(4), drop(8), _(16), value(N) }
    //   => We still have the space to put drop at the end of the padding!
    let layout = Layout::new::<ExternBox<T>>();

    // The ExternBox<T> type only contains one value, we need to adjust for "len" values.
    let size = offset_of!(ExternBox<T>, value) + len * size_of::<T>();

    // SAFETY: Align is a power-of-two because it come from another layout.
    unsafe { Layout::from_size_align_unchecked(size, layout.align()).pad_to_align() }

}

/// This function is only active when debug assert are effective, it checks that the 
/// type id of the stored type is the same as the given pointer's type.
/// 
/// SAFETY: This function is special because its role is to ensure that the stored type
/// id correspond to the given pointer's type, so if this is not guaranteed by the caller
/// then either this function will cause UB, panic or segfault (reading unaccessible 
/// memory). This is a best-effort to catch UB if our logic is flawed.
#[track_caller]
unsafe fn extern_box_debug_assert<T: 'static>(value_ptr: *mut T) {
    let _ = value_ptr;  // To avoid unused if not debug assertions.
    #[cfg(debug_assertions)] 
    unsafe {

        let type_id = value_ptr
            .wrapping_byte_sub(offset_of!(ExternBox<T>, value))
            .wrapping_byte_add(offset_of!(ExternBox<T>, type_id))
            .cast::<TypeId>()
            .read();

        assert_eq!(type_id, TypeId::of::<T>(), "incoherent type id causing unsafe");

    }
}

/// Internal function to dealloc the given extern box. This DOES NOT drop the value.
/// 
/// SAFETY: The given extern box pointer should be pointing to an initialized and valid
/// extern box of that exact type!
unsafe fn extern_box_free_unchecked<T: 'static>(ptr: *mut ExternBox<T>) {
    unsafe {

        let len = ptr
            .byte_add(offset_of!(ExternBox<T>, len))
            .cast::<usize>()
            .read();

        // We can reconstruct the layout because we have the length.
        let layout = extern_box_layout::<T>(len);
        alloc::dealloc(ptr.cast(), layout);

    }
}

/// Drop the given extern-boxed value and then free the full allocation.
/// 
/// SAFETY: The given extern box pointer should be pointing to an initialized and valid
/// extern box's value of that exact type!
pub unsafe fn extern_box_drop_unchecked<T: 'static>(value_ptr: *mut T) {

    /// This guard is internally used to ensure that, despite any panic, the 
    /// allocation will be freed!
    struct FreeGuard<T: 'static>(*mut ExternBox<T>);
    impl<T: 'static> Drop for FreeGuard<T> {
        fn drop(&mut self) {
            // SAFETY: The SAFETY conditions of the super method applies here.
            unsafe { 
                extern_box_free_unchecked(self.0);
            }
        }
    }

    // SAFETY: We know that this points to 'ExternBox<T>.value' and we access the
    // fields safely using offset_of!.
    unsafe { 

        extern_box_debug_assert(value_ptr);
        
        let ptr = value_ptr
            .byte_sub(offset_of!(ExternBox<T>, value))
            .cast::<ExternBox<T>>();

        let len = ptr
            .byte_add(offset_of!(ExternBox<T>, len))
            .cast::<usize>()
            .read();

        let guard = FreeGuard(ptr);
        std::ptr::slice_from_raw_parts_mut(value_ptr, len).drop_in_place();
        drop(guard);

    }

}

/// Allocate a raw extern box, returning the pointer to uninitialized value(s). 
/// The number of values to put in the allocation must be given by 'len'.
#[inline]
fn extern_box_raw<T: 'static>(len: usize) -> *mut T {

    let layout = extern_box_layout::<T>(len);

    // SAFETY: Size can't be one, because we at least have the drop fn pointer.
    let ptr = unsafe { alloc::alloc(layout).cast::<ExternBox<T>>() };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }

    // SAFETY: Read below.
    #[cfg(debug_assertions)] 
    unsafe {
        ptr.byte_add(offset_of!(ExternBox<T>, type_id))
            .cast::<TypeId>()
            .write(TypeId::of::<T>());
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
            .write(extern_box_drop_unchecked::<T>);

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

/// Allocate the given object in a special box that also embed the drop function.
#[inline]
pub fn extern_box_option<T: 'static>(value: Option<T>) -> *mut T {
    match value {
        Some(value) => extern_box(value),
        None => ptr::null_mut(),
    }
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
    // NOTE: We don't directly allocate a 'u8' type, even if this would be correct,
    // because we want to put the right type_id in debug, that correspond to the ptr type.
    let ptr = extern_box_raw::<c_char>(len + 1).cast::<u8>();

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

/// Free the extern box pointing to the given value and return the given value.
/// 
/// SAFETY: You must ensure that the value does point to an extern-boxed value that has
/// no yet been freed nor taken, exactly of the given type.
/// 
/// FIXME: This only works if the extern box contains at least one value in length.
#[inline]
pub unsafe fn extern_box_take<T: 'static>(value_ptr: *mut T) -> T {
    
    debug_assert!(!value_ptr.is_null());

    // SAFETY: This function pre-condition ensure correctness of the reads.
    unsafe {

        extern_box_debug_assert(value_ptr);

        // Start by reading the value, now the value at that position should never be 
        // read again, so we free that function.
        let read = value_ptr.read();

        // Now get the pointer to the extern box we want to free!
        let ptr = value_ptr
            .byte_sub(offset_of!(ExternBox<T>, value))
            .cast::<ExternBox<T>>();

        // We're juste freeing the memory, not dropping the value, because we should not.
        extern_box_free_unchecked(ptr);

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
            assert_eq!(extern_box_layout::<T>(0).size(), offset_of!(ExternBox<T>, value));
            assert_eq!(extern_box_layout::<T>(1).size(), size_of::<ExternBox<T>>());
            assert_eq!(extern_box_layout::<T>(9).size(), size_of::<ExternBox<T>>() + size_of::<T>() * 8);
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
    fn special() {
        assert_eq!(extern_box_option(None::<u32>), ptr::null_mut());
    }

    #[test]
    fn cstr() {

        /// NOTE: c_len don't count nul.
        fn for_str(s: &str, c_len: usize) {
            let cstr = extern_box_cstr_from_str(s);
            let cstr_slice = unsafe { std::slice::from_raw_parts(cstr.cast::<u8>(), c_len + 1) };
            assert_eq!(&cstr_slice[..c_len], &s.as_bytes()[..c_len], "incoherent cstr");
            assert_eq!(cstr_slice[c_len], 0, "missing nul");
            unsafe { extern_box_drop_unchecked(cstr); }
        }

        for_str("Hello world!", 12);
        for_str("Hello world!\0", 12);
        for_str("Hello world!\0rest", 12);

    }

}
