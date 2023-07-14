use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("stack channel send failed")]
    MPSCSendError(#[from] std::sync::mpsc::SendError<crate::tui::canvas::Stack>),

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

    #[error("log error")]
    LogError(#[from] log::SetLoggerError),

    #[error("{source:?}")]
    AnyhowError {
        #[from]
        source: anyhow::Error,
    },

    #[error("{source:?}")]
    TuiError {
        #[from]
        source: crate::tui::error::TuiError,
    },

    #[error("unable to retrieve drawbuffer: {context:?}")]
    UnableToRetrieveSlot { context: String },

    #[error("cannot convert slot to static tile slot")]
    CannotConvertToStatic,

    #[error("cannot convert {idx:?} to sliding tile slot")]
    CannotConvertToSliding { idx: Option<crate::engine::round::Idx> },

    #[error("terminal too small, required minimum size {0} x {1}")]
    TerminalTooSmall(usize, usize),
}
