//! PortableMC bindings for C.
//! 
//! In this library, the naming scheme is simple. All functions that are exported and
//! therefore also defined in the header file are prefixed with `pmc_`, they should use
//! the extern "C" ABI.

#![deny(unsafe_op_in_unsafe_fn)]

pub mod ffi;

pub mod alloc;
pub mod err;

pub mod msa;
