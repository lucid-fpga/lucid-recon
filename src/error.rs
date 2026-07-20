//! Error type. Analysis is fail-soft per file; [`Error`] covers the boundaries
//! that cannot proceed — reading the core directory, loading bundled reference
//! data, and CLI argument handling.

use thiserror::Error;

/// Result alias for the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// A failure that stops recon.
#[derive(Debug, Error)]
pub enum Error {
    /// The MiSTer core directory could not be read.
    #[error("cannot read core at {path}: {source}")]
    Source {
        /// Offending path.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// A bundled reference-data file failed to parse (a build-time bug, not user error).
    #[error("bundled reference data `{name}` is malformed: {source}")]
    RefData {
        /// Which data file.
        name: &'static str,
        /// Underlying serde error.
        source: serde_json::Error,
    },

    /// The CLI was invoked with unusable arguments.
    #[error("usage: {0}")]
    Usage(String),
}
