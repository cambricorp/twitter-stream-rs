use chrono::{TimeZone, UTC};
use futures::{Future, Sink, Stream};
use futures::stream::{self, Then};
use futures::sync::mpsc::{self, Receiver};
use hyper;
use oauthcli::{OAuthAuthorizationHeader, ParseOAuthAuthorizationHeaderError};
use serde::de::{Deserialize, Deserializer, Error};
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead};
use std::str::FromStr;
use std::thread;
use types::DateTime;

/// A stream over each line on a `BufRead`.
pub type Lines = Then<
    Receiver<Result<String, io::Error>>,
    fn(Result<Result<String, io::Error>, ()>) -> Result<String, io::Error>,
    Result<String, io::Error>,
>;

/// Represents an infallible operation in `default_client::new`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Never {}

#[derive(Clone, Debug)]
pub struct OAuthHeaderWrapper(pub OAuthAuthorizationHeader);

macro_rules! string_enums {
    (
        $(
            $(#[$attr:meta])*
            pub enum $E:ident {
                $(
                    $(#[$v_attr:meta])*
                    :$V:ident($by:expr) // The leading (ugly) colon is to suppress local ambiguity error.
                ),*;
                $(#[$u_attr:meta])*
                :$U:ident(_),
            }
        )*
    ) => {
        $(
            $(#[$attr])*
            pub enum $E {
                $(
                    $(#[$v_attr])*
                    $V,
                )*
                $(#[$u_attr])*
                $U(String),
            }

            impl ::serde::Deserialize for $E {
                fn deserialize<D: ::serde::Deserializer>(d: D) -> ::std::result::Result<Self, D::Error> {
                    struct V;

                    impl ::serde::de::Visitor for V {
                        type Value = $E;

                        fn visit_str<E>(self, s: &str) -> ::std::result::Result<$E, E> {
                            match s {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s.to_owned())),
                            }
                        }

                        fn visit_string<E>(self, s: String) -> ::std::result::Result<$E, E> {
                            match s.as_str() {
                                $($by => Ok($E::$V),)*
                                _ => Ok($E::$U(s)),
                            }
                        }

                        fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                            write!(f, "a string")
                        }
                    }

                    d.deserialize_string(V)
                }
            }

            impl ::std::convert::AsRef<str> for $E {
                fn as_ref(&self) -> &str {
                    match *self {
                        $($E::$V => $by,)*
                        $E::$U(ref s) => s,
                    }
                }
            }

            impl ::std::cmp::PartialEq for $E {
                fn eq(&self, other: &$E) -> bool {
                    match *self {
                        $($E::$V => match *other {
                            $E::$V => true,
                            $E::$U(ref s) if $by == s => true,
                            _ => false,
                        },)*
                        $E::$U(ref s) => match *other {
                            $($E::$V => $by == s,)*
                            $E::$U(ref t) => s == t,
                        },
                    }
                }
            }

            impl ::std::hash::Hash for $E {
                fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                    match *self {
                        $($E::$V => $by.hash(state),)*
                        $E::$U(ref s) => s.hash(state),
                    }
                }
            }

            impl ::std::cmp::Eq for $E {}
        )*
    }
}

/// Returns a stream over each line on `a`.
#[allow(unused_variables)]
pub fn lines<A: BufRead + Send + 'static>(a: A) -> Lines {
    let (tx, rx) = mpsc::channel(8); // TODO: is this a proper value?

    thread::spawn(move || {
        let iter = a.lines().map(Ok);
        let stream = stream::iter(iter);
        tx.send_all(stream).wait().is_ok() // Fails silently when `tx` is dropped.
    });

    fn thener(r: Result<Result<String, io::Error>, ()>) -> Result<String, io::Error> {
        r.expect("Receiver failed")
    }

    rx.then(thener as _)
}

pub fn parse_datetime(s: &str) -> ::chrono::format::ParseResult<DateTime> {
    UTC.datetime_from_str(s, "%a %b %e %H:%M:%S %z %Y")
}

pub fn deserialize_datetime<D: Deserializer>(d: D) -> Result<DateTime, D::Error> {
    parse_datetime(&String::deserialize(d)?).map_err(|e| D::Error::custom(e.to_string()))
}

impl ::std::error::Error for Never {
    fn description(&self) -> &str {
        unreachable!()
    }
}

impl Display for Never {
    fn fmt(&self, _: &mut Formatter) -> fmt::Result {
        unreachable!()
    }
}

impl FromStr for OAuthHeaderWrapper {
    type Err = ParseOAuthAuthorizationHeaderError;

    fn from_str(s: &str) -> Result<Self, ParseOAuthAuthorizationHeaderError> {
        Ok(OAuthHeaderWrapper(OAuthAuthorizationHeader::from_str(s)?))
    }
}

impl hyper::header::Scheme for OAuthHeaderWrapper {
    fn scheme() -> Option<&'static str> {
        Some("OAuth")
    }

    fn fmt_scheme(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.0.auth_param())
    }
}
