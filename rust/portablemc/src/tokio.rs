//! Async utilities around Tokio runtime.

use std::future::Future;


/// Block on the given future with the Tokio runtime with time and I/O enabled.
pub fn sync<F: Future>(future: F) -> F::Output {
    
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();

    rt.block_on(future)
    
}
