use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, TuiError>;

pub struct TuiError {
    bt: std::backtrace::Backtrace,
    pub(crate) inner: InnerError,
}

impl std::fmt::Debug for TuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{0:?}\n{1}", self.inner, self.bt)
    }
}

impl std::fmt::Display for TuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{0:?}\n{1}", self.inner, self.bt)
    }
}

impl std::error::Error for TuiError {}

impl From<std::sync::mpsc::SendError<crate::tui::geometry::Idx>> for TuiError {
    fn from(inner: std::sync::mpsc::SendError<crate::tui::geometry::Idx>) -> TuiError {
        InnerError::IdxSendError(inner).into()
    }
}

impl From<std::sync::mpsc::SendError<crate::tui::tuxel::Tuxel>> for TuiError {
    fn from(inner: std::sync::mpsc::SendError<crate::tui::tuxel::Tuxel>) -> TuiError {
        InnerError::TuxelSendError(inner).into()
    }
}

impl From<std::io::Error> for TuiError {
    fn from(inner: std::io::Error) -> TuiError {
        InnerError::StdIOError(inner).into()
    }
}

impl From<anyhow::Error> for TuiError {
    fn from(inner: anyhow::Error) -> TuiError {
        InnerError::AnyhowError { source: inner }.into()
    }
}

impl From<InnerError> for TuiError {
    fn from(inner: InnerError) -> Self {
        Self {
            bt: std::backtrace::Backtrace::capture(),
            inner,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum InnerError {
    #[error("cell already owned")]
    CellAlreadyOwned,

    #[error("{0}ward translation impossible")]
    TranslationImpossible(super::geometry::Direction),

    #[error("out of bounds x - {0}")]
    OutOfBoundsX(usize),

    #[error("out of bounds y - {0}")]
    OutOfBoundsY(usize),

    #[error("out of bounds z - {0}")]
    OutOfBoundsZ(usize),

    #[error("idx channel send failed")]
    IdxSendError(#[from] std::sync::mpsc::SendError<crate::tui::geometry::Idx>),

    #[error("tuxel channel send failed")]
    TuxelSendError(#[from] std::sync::mpsc::SendError<crate::tui::tuxel::Tuxel>),

    #[error(
        "invalid translation \n\tmagnitude: {mag:?} \n\tdirection: {dir:?} \n\trectangle: {rect:?}"
    )]
    InvalidVectorTranslation {
        mag: usize,
        dir: super::geometry::Direction,
        rect: super::geometry::Rectangle,
    },

    #[error("top tuxel in stack not found")]
    TopTuxelNotFound,

    #[error("drawbuffer translation failed: {0}")]
    DrawBufferTranslationFailed(String),

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

    #[error("{source:?}")]
    AnyhowError {
        #[from]
        source: anyhow::Error,
    },

    #[error("exceeded retry limit for locking drawbuffer: {0:?}")]
    ExceedRetryLimitForLockingDrawBuffer(String),

    #[error("rectangle dimensions must match")]
    RectangleDimensionsMustMatch,
}
