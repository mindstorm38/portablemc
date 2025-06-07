//! PortableMC is a library and CLI for programmatically launching Minecraft.

#![deny(unsafe_op_in_unsafe_fn)]

mod path;
mod http;
mod tokio;
mod serde;

pub mod download;

pub mod maven;

pub mod msa;

pub mod base;
pub mod mojang;
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


/// This macro help defining an event handler trait, this macro automatically implements 
/// the trait for any `&mut impl Self`, every function has a default empty body so that
/// any addition of method is backward compatible and valid for minor version increment.
macro_rules! trait_event_handler {
    (
        $( #[ $meta:meta ] )*
        $vis:vis trait $name:ident $( : $( $super:path ),+ $(,)? )? {
            $( 
                $( #[ $func_meta:meta ] )* 
                fn $func:ident ( $( $arg:ident : $arg_ty:ty ),* $(,)? ) 
                $( -> $ret_ty:ty = $ret_default:expr )?; 
            )*
        }
    ) => {

        $( #[ $meta ] )*
        $vis trait $name $( : $( $super ),+ )? {

            /// This special handler function can be used to provide a fallback for every
            /// function that is not implemented by the implementor.
            /// 
            /// This function is exposed in the public API but it's unsure if it will be
            /// implemented as-is in the future, so it cannot be implemented nor called 
            /// by external crates thanks to a "sealed" token type.
            #[doc(hidden)]
            #[inline(always)]
            fn __internal_fallback(&mut self, _token: $crate::sealed::Token) -> Option<&mut dyn $name> {
                None
            }

            $( 
                $( #[ $func_meta ] )* 
                fn $func ( &mut self $( , $arg : $arg_ty )* ) $( -> $ret_ty )? {
                    // We expect the fallback call to be inlined every time because the
                    // default functions are statically known, and for the dynamic 
                    // dispatch implementation with '&mut dyn H' (below) all functions 
                    // are defined to just forward the call, so the fallback function is
                    // never used.
                    if let Some(fallback) = $name::__internal_fallback(self, $crate::sealed::Token) {
                        $name::$func( fallback $(, $arg)* )
                    } else {
                        $( $ret_default )?
                    }
                }
            )*

        }

        impl $name for () {  }

        impl<H: $name + ?Sized> $name for &'_ mut H {
            $( 
                fn $func ( &mut self $( , $arg : $arg_ty )* ) $( -> $ret_ty )? {
                    $name::$func( &mut **self $(, $arg)* )
                }
            )*
        }

        // Implementation for tuples, calling both handlers each time.
        impl<H0: $name, H1: $name> $name for (H0, H1) {
            $( 
                fn $func ( &mut self $( , $arg : $arg_ty )* ) $( -> $ret_ty )? {
                    $name::$func( &mut self.0 $(, $arg)* );
                    $name::$func( &mut self.1 $(, $arg)* )  // We only keep last value.
                }
            )*
        }

    };
}

pub(crate) use trait_event_handler;
