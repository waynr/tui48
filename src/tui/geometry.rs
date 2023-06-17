/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Default)]
pub(crate) struct Idx(pub usize, pub usize, pub usize);

#[derive(Clone, Default)]
pub(crate) struct Bounds2D(pub usize, pub usize);

#[derive(Clone, Default)]
pub(crate) struct Rectangle(pub Idx, pub Bounds2D);

impl Rectangle {
    pub(crate) fn width(&self) -> usize {
        self.1 .0
    }

    pub(crate) fn height(&self) -> usize {
        self.1 .1
    }

    pub(crate) fn x(&self) -> usize {
        self.0 .0
    }

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

    pub(crate) fn extents(&self) -> (usize, usize) {
        (self.0 .0 + self.1 .0, self.0 .1 + self.1 .1)
    }
}

pub(crate) enum Position {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Idx(usize, usize),
}

