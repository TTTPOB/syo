//! Typed HTTP client for the SiYuan kernel.
pub mod api;
pub mod client;
pub mod escape;
pub mod response;
pub use client::SiyuanClient;
pub use escape::{MAX_SEARCH_LIMIT, escape_sql_string};
