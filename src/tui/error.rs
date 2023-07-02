use thiserror;

/// The Result type for tui48.
pub(crate) type Result<T> = std::result::Result<T, TuiError>;

#[derive(thiserror::Error, Debug)]
pub(crate) enum TuiError {
    #[error("terminal too small, required minimum size {0} x {1}")]
    TerminalTooSmall(usize, usize),

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

    #[error("invalid translation \n\tmagnitude: {mag:?} \n\tdirection: {dir:?} \n\trectangle: {rect:?}")]
    InvalidVectorTranslation {
        mag: usize,
        dir: super::geometry::Direction,
        rect: super::geometry::Rectangle,
    },

    #[error("top tuxel in stack not found")]
    TopTuxelNotFound,

    #[error("io error")]
    StdIOError(#[from] std::io::Error),

    #[error("{source:?}")]
    AnyhowError {
        #[from]
        source: anyhow::Error,
    },
}
