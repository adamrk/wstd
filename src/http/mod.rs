//! HTTP networking support
//!
pub use http::status::StatusCode;
pub use http::uri::{Authority, PathAndQuery, Uri};

#[doc(inline)]
pub use body::{Body, util::BodyExt};
pub use client::Client;
pub use error::{Error, ErrorCode, Result};
pub use fields::{HeaderMap, HeaderName, HeaderValue};
pub use method::Method;
pub use request::Request;
pub use response::Response;
pub use scheme::{InvalidUri, Scheme};

#[cfg(target_env = "p2")]
pub mod body;
#[cfg(target_env = "p3")]
#[path = "body_p3.rs"]
pub mod body;

mod client;
pub mod error;
mod fields;
mod method;
pub mod request;
pub mod response;
mod scheme;
pub mod server;
