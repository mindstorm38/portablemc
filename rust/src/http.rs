//! HTTP wrappers around reqwest, for adding default and common configuration to the 
//! client builder.

use reqwest::{Client, ClientBuilder};

/// The user agent to be used on each HTTP request.
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Get a new client builder for HTTP request.
pub fn builder() -> ClientBuilder {
    Client::builder().user_agent(USER_AGENT)
}
