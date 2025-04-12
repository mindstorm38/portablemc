//! Memory allocation management through the FFI boundaries, allowing a unified free 
//! function for every object.

use std::alloc::{self, Layout, handle_alloc_error};
use std::ffi::{c_char, c_void};
use std::mem::offset_of;
use std::ptr;


// The layout of our extern box is defined as follow. Its structure is not trivial 
// because we have to know call some generic drop glue and we must support any alignment
// that may be required by T, this make will dynamically alter the alignment of the whole
// extern box layout and this is why we need padding.
// 
//     <       HEAD        >
//    +----~~  ~~----+------+---------~~
//    | PAD/RESERVED | DROP | VALUE   ~~
//    +----~~  ~~----+------+---------~~
//                          ^ We return that pointer through the FFI
// 
// VALUE    | T
// DROP     | fn(*mut T)
// PAD      | Depending on the alignment of VALUE and DROP, padding might be needed to 
//          |  ensure that VALUE and DROP are properly aligned.
// RESERVED | Any implementation of the extern box is free to use the padding space, or
//          |  more, for storing any data it wants.
//
// The drop function is responsible for dropping the value, in place, given its pointer,
// and then deallocate the whole structure. Deallocation must happen despite any panics.

/// Internal type alias for the drop function pointer.
type DropFn<T> = fn(value_ptr: *mut T);

/// Statically known drop function pointer size, same regardless of the type.
const DROP_SIZE: usize = size_of::<DropFn<u8>>();

/// Allocate the given object in a special box that also embed the drop function.
#[inline]
pub fn extern_box<T>(value: T) -> *mut T {

    /// Internal function that, depending on a type, returns the full layout of the 
    /// allocation and the size of the (optionally padded) head.
    fn layout<T>() -> (Layout, usize) {

        // Compute the alignment required for the whole structure. Note that we use 
        // the _size of_ drop fn instead of its alignment, because we want to ensure
        // that the alignment will be the same as its size for simplification of pad.
        let align = usize::max(DROP_SIZE, align_of::<T>());

        // Alignment is always a power of two, therefore if the object's alignment is greater,
        // then the required padding must be a multiple of drop fn's size (and so alignment).
        // Here we jut pad the drop fn, this padding, multiple of its alignment, will be 
        // placed before the drop fn.
        let align_mask = align - 1;
        let head_size = (DROP_SIZE + align_mask) & !align_mask;
        let size = head_size + size_of::<T>();
        
        // SAFETY: The alignment is a power of two.
        let layout = unsafe { Layout::from_size_align_unchecked(size, align).pad_to_align() };
        (layout, head_size)

    }

    /// Internal drop implementation for that type.
    fn drop_impl<T>(value_ptr: *mut T) {

        /// This guard is internally used to ensure that, despite any panic, the 
        /// allocation will be freed!
        struct DeallocGuard<T>(*mut T);
        impl<T> Drop for DeallocGuard<T> {
            fn drop(&mut self) {

                // Now we need to deallocate the whole structure!
                let (layout, head_size) = layout::<T>();

                unsafe { 
                    // SAFETY: If this drop implementation exists, it means that the layout
                    // has been validated before allocation and therefore that there is a
                    // head of the given size before the actual object.
                    let ptr = self.0.cast::<u8>().byte_sub(head_size);
                    alloc::dealloc(ptr, layout);
                }

            }
        }

        // Create the guard here so we ensure that this is deallocated in any case!
        let guard = DeallocGuard(value_ptr);
        // We know that this is initialized code.
        unsafe { value_ptr.drop_in_place(); }
        // Not needed but it helps understanding.
        drop(guard);

    }

    let (layout, head_size) = layout::<T>();

    // SAFETY: We safely created the layout.
    let ptr = unsafe { alloc::alloc(layout) };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }

    unsafe { 
        
        // SAFETY: Our layout is made so that the value is in allocated object.
        let value_ptr = ptr.byte_add(head_size).cast::<T>();
        debug_assert!(value_ptr.is_aligned());
        value_ptr.write(value);

        // SAFETY: Out layout is made so that the drop fn pointer is stored just before value.
        let drop_ptr = value_ptr.cast::<DropFn<T>>().sub(1);
        debug_assert!(drop_ptr.is_aligned());
        drop_ptr.write(drop_impl::<T>);

        value_ptr

    }

}

/// Allocate a C-string from a given Rust string slice.
#[inline]
pub fn extern_box_cstr_from_str(s: &str) -> *const c_char {

    #[repr(C)]
    struct Head {
        size: usize,
        drop: DropFn<u8>,
        first_byte: u8,
    }

    const SIZE_OFFSET: usize = offset_of!(Head, size);
    const STR_OFFSET: usize = offset_of!(Head, first_byte);
    
    /// Internal drop implementation for that type.
    fn drop_impl(value_ptr: *mut u8) {

        unsafe {
            
            // SAFETY: We go to the first byte of the allocated object.
            let head_ptr = value_ptr.cast::<Head>().sub(1);

            // SAFETY: We goto a known field in the allocated head.
            let size_ptr = head_ptr.byte_add(SIZE_OFFSET).cast::<usize>();
            debug_assert!(size_ptr.is_aligned());
            let size = size_ptr.read();

            // SAFETY: Alignment come from align_of
            let layout = Layout::from_size_align_unchecked(size, align_of::<Head>());
            
            // SAFETY: Layout has been reconstructed.
            alloc::dealloc(head_ptr.cast(), layout);

        };

    }

    // We immediately treat the input string as bytes, because this is what they are
    // in C, so if there are interior nul bytes, we truncate them.
    let bytes = s.as_bytes();
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(s.len());

    // A string has an alignment of 1, and we know that our drop function has necessarily
    // a greater alignment, so we just use its alignment and there will never be any
    // padding. However, we use the 'reserved' space before the drop fn to store the
    // size of the whole allocation (don't need to store the alignment because it's 
    // known statically).
    // NOTE: +1 for null at the end.
    let size = STR_OFFSET + len + 1;
    // SAFETY: Alignment come from another layout.
    let layout = unsafe { Layout::from_size_align_unchecked(size, align_of::<Head>()) };

    // SAFETY: We safely created the layout, thus making the cast safe.
    let ptr = unsafe { alloc::alloc(layout) };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }
    
    unsafe { 
        
        // SAFETY: We have space to write that head.
        let head_ptr = ptr.cast::<Head>();
        debug_assert!(head_ptr.is_aligned());
        head_ptr.write(Head { 
            size, 
            drop: drop_impl, 
            first_byte: 0,  // It's always safe to write at least the nul.
        });

        // SAFETY: We move to the end of the head and the beginning of the string so we're
        // in the same allocated object.
        let cstr_ptr = head_ptr.cast::<u8>().byte_add(STR_OFFSET);
        // SAFETY: The newly allocated object can't overlap with the string we copy.
        ptr::copy_nonoverlapping(bytes.as_ptr(), cstr_ptr, len);
        // SAFETY: The last nul is inside the allocated object, we can write it.
        cstr_ptr.byte_add(len).write(0);

        cstr_ptr.cast()

    }

}


// =======
// Bindings
// =======

#[no_mangle]
pub extern "C" fn pmc_free(value: *mut c_void) {

    // Ignore null pointers, this can be used to simplify some code.
    if value.is_null() {
        return;
    }

    unsafe {
        
        // SAFETY: The functions above that allocate "ExternBox" should put the drop 
        // function just before the value. We can read this pointer and call it!
        // This function is responsible for dropping and deallocation.
        let drop_ptr = value.cast::<DropFn<u8>>().sub(1);
        let drop_fn = drop_ptr.read();
        drop_fn(value.cast());

    }

}
