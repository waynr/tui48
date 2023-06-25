use super::error::{Result, TuiError};

/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Default)]
pub(crate) struct Idx(pub usize, pub usize, pub usize);

impl Idx {
    #[inline(always)]
    pub(crate) fn x(&self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) fn y(&self) -> usize {
        self.1
    }

    #[inline(always)]
    pub(crate) fn z(&self) -> usize {
        self.2
    }
}

#[derive(Clone, Default)]
pub(crate) struct Bounds2D(pub usize, pub usize);

#[derive(Clone, Default)]
pub(crate) struct Rectangle(pub Idx, pub Bounds2D);

impl Rectangle {
    #[inline(always)]
    pub(crate) fn width(&self) -> usize {
        self.1 .0
    }

    #[inline(always)]
    pub(crate) fn height(&self) -> usize {
        self.1 .1
    }

    #[inline(always)]
    pub(crate) fn x(&self) -> usize {
        self.0 .0
    }

    #[inline(always)]
    pub(crate) fn y(&self) -> usize {
        self.0 .1
    }

    pub(crate) fn relative_idx(&self, pos: &Position) -> (usize, usize) {
        match pos {
            Position::TopLeft => (0, 0),
            Position::TopRight => (self.width() - 1, 0),
            Position::BottomLeft => (0, self.height() - 1),
            Position::BottomRight => (self.width() - 1, self.height() - 1),
            Position::Idx(x, y) => (*x, *y),
        }
    }

    #[inline(always)]
    pub(crate) fn extents(&self) -> (usize, usize) {
        (self.0 .0 + self.1 .0, self.0 .1 + self.1 .1)
    }

    #[inline(always)]
    pub(crate) fn contains_or_err(&self, idx: &Idx) -> Result<()> {
        if idx.x() < self.x() || idx.x() > self.x() + self.width() {
            return Err(TuiError::OutOfBoundsY(idx.x()))
        }
        if idx.y() < self.y() || idx.y() > self.y() + self.height() {
            return Err(TuiError::OutOfBoundsY(idx.y()))
        }
        Ok(())
    }
}

pub(crate) enum Position {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Idx(usize, usize),
}

/// Direction represents the direction indicated by the player.
#[derive(Clone, Debug, Default)]
pub(crate) enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        };
        write!(f, "{}", s)
    }
}
