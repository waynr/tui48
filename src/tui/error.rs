use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, TuiError>;

#[derive(thiserror::Error, Debug)]
pub(crate) enum TuiError {
    #[error("terminal too small, required minimum size {0} x {1}")]
    TerminalTooSmall(usize, usize),

    #[error("cell already owned")]
    CellAlreadyOwned,

    #[error("stack channel send failed")]
    MPSCSendError(#[from] std::sync::mpsc::SendError<crate::tui::geometry::Idx>),

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

    #[error("{source:?}")]
    AnyhowError {
        #[from]
        source: anyhow::Error,
    },
}
