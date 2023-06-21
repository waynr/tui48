use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, TuiError>;

#[derive(thiserror::Error, Debug)]
pub(crate) enum TuiError {
    #[error("terminal too small, required minimum size {0} x {1}")]
    TerminalTooSmall(usize, usize),

    #[error("cell already owned")]
    CellAlreadyOwned,

    #[error("idx channel send failed")]
    IdxSendError(#[from] std::sync::mpsc::SendError<crate::tui::geometry::Idx>),

    #[error("tuxel channel send failed")]
    TuxelSendError(#[from] std::sync::mpsc::SendError<crate::tui::tuxel::Tuxel>),

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

    #[error("{source:?}")]
    AnyhowError {
        #[from]
        source: anyhow::Error,
    },
}
