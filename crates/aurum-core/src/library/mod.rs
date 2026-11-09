//! Music library: filesystem scanner, SQLite store and cover art loading.

pub mod cover;
mod db;
pub mod scan;

pub use cover::Cover;
pub use db::{Album, Library};
pub use scan::{ScanEvent, ScanSummary, Scanner};

/// Errors from the library database and scanner setup.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("database schema version {0} is newer than this build supports")]
    SchemaTooNew(i32),
}
