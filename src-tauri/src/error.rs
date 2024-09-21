#![allow(unused)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PIVXErrors {
    #[error("Failed to fetch data")]
    FetchError(#[from] reqwest::Error),

    #[error("Server returned a non-ok status code")]
    ServerError,

    #[error("No data directory found")]
    NoDataDir,

    #[error("Failed to create file")]
    CreateFileError(#[from] std::io::Error),

    #[error("Pivxd not found")]
    PivxdNotFound,

    #[error("Invalid sha256 sum")]
    WrongSha256Sum(Option<std::io::Error>),

    #[error("Unable to connect to pivxd")]
    UnableToAuthPIVXD(#[from] jsonrpsee::core::ClientError),

    #[error("Error with Sqlite")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Tauri error")]
    TauriError(#[from] tauri::Error),
}

pub type Result<T> = std::result::Result<T, PIVXErrors>;
