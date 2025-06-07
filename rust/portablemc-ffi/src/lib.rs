//! PortableMC FFI bindings for external languages such as C.
//! 
//! The goal is to have an extensible and as complete as possible C interface to allow
//! any other language to bind onto it, because almost all languages can bind to a C
//! (shared) object.
//! 
//! In this library, the naming scheme is simple. All types and functions that are 
//! exported and therefore also defined in the header file are prefixed with `pmc_`, 
//! they should use the extern "C" ABI. For opaque types that are implementation details,
//! they are prefixed with "Extern".
#![deny(unsafe_op_in_unsafe_fn)]

pub mod raw;

pub mod cstr;
pub mod alloc;
pub mod err;

pub mod msa;

pub mod base;
