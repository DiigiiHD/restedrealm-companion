//! RestedRealm Companion core.
//!
//! Reads the addon's completed SavedVariables as data, keeps a private local
//! queue and uploads to restedrealm.com. It is a port of the Python companion in
//! `companion/`, which stays the reference until this crate replaces it: the
//! same queue file, the same record digests and the same Windows credential.

pub mod canon;
pub mod credentials;
pub mod game;
pub mod lua;
pub mod queue;
pub mod save;
pub mod upload;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The save is not the addon's data, is incomplete or breaks a limit.
    #[error("{0}")]
    SaveFormat(String),
    /// A safety check refused the action; nothing was changed.
    #[error("{0}")]
    Refused(String),
    /// The website refused or did not answer clearly; records stay queued.
    #[error("{0}")]
    Upload(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("local queue: {0}")]
    Database(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
