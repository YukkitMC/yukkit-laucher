//! Turns technical failures into messages a normal player can act on.

use serde::Serialize;
use std::fmt;

/// What the UI shows when something fails: a short title, a plain-English
/// explanation, and the technical details hidden behind "Show details".
#[derive(Debug, Serialize, Clone)]
pub struct UiError {
    pub title: String,
    pub message: String,
    pub details: String,
}

impl UiError {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            details: String::new(),
        }
    }
}

/// An error whose wording we chose on purpose. Attach it with [`FriendlyExt::friendly`].
#[derive(Debug, Clone)]
pub struct Friendly {
    pub title: String,
    pub message: String,
    pub network: bool,
}

impl Friendly {
    pub fn new(title: &str, message: &str) -> Self {
        Self { title: title.into(), message: message.into(), network: false }
    }

    pub fn network(title: &str, message: &str) -> Self {
        Self { title: title.into(), message: message.into(), network: true }
    }
}

impl fmt::Display for Friendly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.title, self.message)
    }
}

impl std::error::Error for Friendly {}

pub fn friendly(title: &str, message: &str) -> anyhow::Error {
    anyhow::Error::new(Friendly::new(title, message))
}

pub trait FriendlyExt<T> {
    fn friendly(self, title: &str, message: &str) -> anyhow::Result<T>;
}

impl<T, E: Into<anyhow::Error>> FriendlyExt<T> for Result<T, E> {
    fn friendly(self, title: &str, message: &str) -> anyhow::Result<T> {
        self.map_err(|e| e.into().context(Friendly::new(title, message)))
    }
}

fn find_friendly(err: &anyhow::Error) -> Option<Friendly> {
    if let Some(f) = err.downcast_ref::<Friendly>() {
        return Some(f.clone());
    }
    err.chain().find_map(|c| c.downcast_ref::<Friendly>().cloned())
}

fn find_reqwest(err: &anyhow::Error) -> Option<&reqwest::Error> {
    if let Some(e) = err.downcast_ref::<reqwest::Error>() {
        return Some(e);
    }
    err.chain().find_map(|c| c.downcast_ref::<reqwest::Error>())
}

fn find_io(err: &anyhow::Error) -> Option<&std::io::Error> {
    if let Some(e) = err.downcast_ref::<std::io::Error>() {
        return Some(e);
    }
    err.chain().find_map(|c| c.downcast_ref::<std::io::Error>())
}

/// True when the failure was caused by the network (so a cached install can still be started).
pub fn is_network(err: &anyhow::Error) -> bool {
    find_friendly(err).map(|f| f.network).unwrap_or(false) || find_reqwest(err).is_some()
}

impl From<anyhow::Error> for UiError {
    fn from(err: anyhow::Error) -> Self {
        let details = format!("{:#}", err);

        if let Some(f) = find_friendly(&err) {
            return UiError { title: f.title, message: f.message, details };
        }

        if let Some(e) = find_reqwest(&err) {
            let (title, message) = if e.is_timeout() || e.is_connect() || e.is_request() {
                (
                    "No connection",
                    "The launcher couldn't reach the download servers. Check that you're online, and that a firewall, VPN or antivirus isn't blocking the launcher, then try again.",
                )
            } else if e.status().map(|s| s.is_server_error()).unwrap_or(false) {
                (
                    "A download server is having trouble",
                    "One of the download servers isn't answering properly right now. This is usually temporary — wait a minute and try again.",
                )
            } else {
                (
                    "A download didn't finish",
                    "Something went wrong while downloading game files. Try again — files that already finished won't be downloaded twice.",
                )
            };
            return UiError { title: title.into(), message: message.into(), details };
        }

        if let Some(e) = find_io(&err) {
            let disk_full = matches!(e.raw_os_error(), Some(112) | Some(28));
            let (title, message) = if disk_full {
                (
                    "Your drive is full",
                    "There isn't enough free space to install the game. Free up a few gigabytes, or pick another install location in Settings.",
                )
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                (
                    "The launcher can't write its files",
                    "Windows blocked the launcher from writing to the install folder. Close Minecraft if it's open, or choose a different install location in Settings.",
                )
            } else {
                (
                    "A file problem stopped the install",
                    "The launcher couldn't read or write one of its files. Try again; if it keeps happening, choose a different install location in Settings.",
                )
            };
            return UiError { title: title.into(), message: message.into(), details };
        }

        UiError {
            title: "Something went wrong".into(),
            message: "The launcher ran into an unexpected problem. Try again, and if it keeps happening, copy the details below when asking for help.".into(),
            details,
        }
    }
}
