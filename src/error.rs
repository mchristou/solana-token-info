use std::num::ParseIntError;
use thiserror::Error;
use tokio::task::JoinError;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to parse Int: {0}")]
    ParseInt(#[from] ParseIntError),

    #[error("Solana client: {0}")]
    SolanaClient(#[from] solana_client::client_error::ClientError),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Error: {0}")]
    Generic(String),

    #[error("Join: {0}")]
    Join(#[from] JoinError),
}
