use thiserror;

/// The Result type for tui48.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("out of bounds x - {0}")]
    OutOfBoundsX(usize),
    #[error("out of bounds y - {0}")]
    OutOfBoundsY(usize),
    #[error("out of bounds z - {0}")]
    OutOfBoundsZ(usize),
}
