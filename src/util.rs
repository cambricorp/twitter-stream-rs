use futures::{Future, Poll, Sink, Stream};
use futures::stream::{self, Then};
use futures::sync::mpsc::{self, Receiver};
use hyper;
use oauthcli::{OAuthAuthorizationHeader, ParseOAuthAuthorizationHeaderError};
use std::fmt;
use std::io::BufRead;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use Error;

/// A stream over each line on a `BufRead`.
pub type Lines = Then<
    Receiver<Result<String, Error>>,
    fn(Result<Result<String, Error>, ()>) -> Result<String, Error>,
    Result<String, Error>,
>;

#[derive(Clone, Debug)]
pub struct OAuthHeaderWrapper(pub OAuthAuthorizationHeader);

/// A future which resolves at a specific period of time.
pub struct Timeout {
    when: Instant,
    parked: bool,
    is_active: Arc<AtomicBool>,
}

/// Returns a stream over each line on `a`.
#[allow(unused_variables)]
pub fn lines<A: BufRead + Send + 'static>(a: A) -> Lines {
    let (tx, rx) = mpsc::channel(8); // TODO: is this a proper value?

    thread::spawn(move || {
        let iter = a.lines().map(|r| Ok(r.map_err(Error::Io)));
        let stream = stream::iter(iter);
        tx.send_all(stream).wait().is_ok() // Fails silently when `tx` is dropped.
    });

    let rx = rx.then(thener as _);
    fn thener(r: Result<Result<String, Error>, ()>) -> Result<String, Error> {
        r.expect("Receiver failed")
    }

    rx
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

impl Timeout {
    pub fn after(dur: Duration) -> Self {
        Timeout::at(Instant::now() + dur)
    }

    pub fn at(at: Instant) -> Self {
        Timeout {
            when: at,
            parked: false,
            is_active: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn park(&mut self, now: Instant) {
        use futures::task;

        if !self.parked {
            let wait = self.when - now; // Panics if `self.when < now`.
            let is_active = self.is_active.clone();
            let task = task::park();
            thread::spawn(move || {
                thread::sleep(wait);
                if is_active.load(Ordering::Relaxed) {
                    task.unpark();
                }
            });
            self.parked = true;
        }
    }
}

impl Future for Timeout {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        use futures::Async::*;

        let now = Instant::now();
        if now < self.when {
            self.park(now);
            Ok(NotReady)
        } else {
            Ok(Ready(()))
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
    }
}
