use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("stack channel send failed")]
    MPSCSendError(#[from] std::sync::mpsc::SendError<crate::tui::canvas::Stack>),

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

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

    #[error("default colors already set")]
    DefaultColorsAlreadySet,

    #[error("unable to retrieve drawbuffer: {context:?}")]
    UnableToRetrieveSlot { context: String },

    #[error("unexpected strong reference in smart pointer")]
    UnexpectedStrongReference,

    #[error("cannot convert slot to sliding tile slot")]
    CannotConvertToSliding,

    #[error("cannot convert slot to new sliding tile slot")]
    CannotConvertToNewSlidingTileSlot,
}
