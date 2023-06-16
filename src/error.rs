use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("out of bounds x - {0}")]
    OutOfBoundsX(usize),
    #[error("out of bounds y - {0}")]
    OutOfBoundsY(usize),
    #[error("out of bounds z - {0}")]
    OutOfBoundsZ(usize),

    #[error("terminal too small, required minimum size {0} x {1}")]
    TerminalTooSmall(usize, usize),

    #[error("stack channel send failed")]
    MPSCSendError(#[from] std::sync::mpsc::SendError<crate::tui::canvas::Stack>),

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

    #[error("{source:?}")]
    AnyhowError {
        #[from]
        source: anyhow::Error,
    },
}
