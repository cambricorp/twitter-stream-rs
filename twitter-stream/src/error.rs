//! Error types

pub use hyper::Error as HyperError;
pub use json::Error as JsonError;

use message::Disconnect;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::str::Utf8Error;
use types::StatusCode;

/// An error occurred while trying to connect to a Stream.
#[derive(Debug)]
pub enum Error {
    /// The Stream has been disconnected by the server.
    Disconnect(Box<Disconnect>),
    /// An HTTP error from the Stream.
    Http(StatusCode),
    /// An error from the `hyper` crate.
    Hyper(HyperError),
    /// Failed to parse a JSON message from a Stream.
    Json(JsonError),
    TimedOut,
    Utf8(Utf8Error),
    Custom(Box<StdError + Send + Sync>),
}

impl Error {
    pub fn custom<E>(error: E) -> Self where Box<StdError + Send + Sync>: From<E> {
        Error::Custom(From::from(error))
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Disconnect(ref d) => &d.reason,
            Http(ref status) => status.canonical_reason().unwrap_or("<unknown status code>"),
            Hyper(ref e) => e.description(),
            Json(ref e) => e.description(),
            TimedOut => "timed out",
            Utf8(ref e) => e.description(),
            Custom(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use Error::*;

        match *self {
            Hyper(ref e) => Some(e),
            Json(ref e) => Some(e),
            Utf8(ref e) => Some(e),
            Custom(ref e) => Some(e.as_ref()),
            Disconnect(_) | Http(_) | TimedOut => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use Error::*;

        match *self {
            Disconnect(ref d) => Display::fmt(d, f),
            Http(ref code) => Display::fmt(code, f),
            Hyper(ref e) => Display::fmt(e, f),
            Json(ref e) => Display::fmt(e, f),
            TimedOut => Display::fmt(self.description(), f),
            Utf8(ref e) => Display::fmt(e, f),
            Custom(ref e) => Display::fmt(e, f),
        }
    }
}
