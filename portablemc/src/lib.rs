//! PortableMC is a library and CLI for programmatically launching Minecraft.

#![deny(unsafe_op_in_unsafe_fn)]

mod path;
mod http;
mod tokio;
mod serde;

pub mod maven;

pub mod msa;

pub mod download;

pub mod base;
pub mod moj;
pub mod fabric;
pub mod forge;


/// Internal module used for sealing traits and their methods with a sealed token.
#[allow(unused)]
mod sealed {

    /// Internal sealed trait that be extended from by traits to be sealed.
    pub trait Sealed {  }

    /// A token type that can be added as a parameter on a function on a non-sealed trait
    /// to make this particular function sealed and not callable nor implementable by 
    /// external crates.
    pub struct Token;

}
