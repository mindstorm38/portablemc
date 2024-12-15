//! This module provides various HTTP(S) request utilities, everything is based on 
//! async reqwest with tokio.

use once_cell::sync::OnceCell;
use reqwest::{Client, ClientBuilder};


/// The user agent to be used on each HTTP request.
pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Get a new client builder for async HTTP(S) requests.
pub fn builder() -> ClientBuilder {
    Client::builder().user_agent(USER_AGENT)
}

/// Return the singleton instance for the HTTP client to be used internally by PMC.
pub fn client() -> reqwest::Result<Client> {
    static INSTANCE: OnceCell<Client> = OnceCell::new();
    let inst = INSTANCE.get_or_try_init(|| {
        builder().build()
    })?;
    Ok(inst.clone())
}
