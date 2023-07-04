use super::error::{Result, TuiError};

/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Bounds2D(pub usize, pub usize);

impl Bounds2D {
    #[inline(always)]
    pub(crate) fn width(&self) -> usize {
        self.0
    }

    #[inline(always)]
    pub(crate) fn height(&self) -> usize {
        self.1
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
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
    pub(crate) fn z(&self) -> usize {
        self.0 .2
    }

    #[inline(always)]
    pub(crate) fn y(&self) -> usize {
        self.0 .1
    }

    #[inline(always)]
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
    pub(crate) fn translate(&mut self, mag: usize, dir: &Direction) -> Result<()> {
        match dir {
            Direction::Left if self.x() > mag => self.0 .0 -= mag,
            Direction::Left if self.x() <= mag => self.0 .0 = 0,
            Direction::Right => self.0 .0 += mag,
            Direction::Up if self.y() > mag => self.0 .1 -= mag,
            Direction::Up if self.y() <= mag => self.0 .1 = 0,
            Direction::Down => self.0 .1 += mag,
            _ => {
                return Err(TuiError::InvalidVectorTranslation {
                    mag,
                    dir: dir.clone(),
                    rect: self.clone(),
                })
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn extents(&self) -> (usize, usize) {
        (self.0 .0 + self.1 .0, self.0 .1 + self.1 .1)
    }

    #[inline(always)]
    pub(crate) fn contains_or_err(&self, idx: &Idx) -> Result<()> {
        if idx.x() < self.x() || idx.x() > self.x() + self.width() {
            return Err(TuiError::OutOfBoundsX(idx.x()));
        }
        if idx.y() < self.y() || idx.y() > self.y() + self.height() {
            return Err(TuiError::OutOfBoundsY(idx.y()));
        }
        Ok(())
    }
}

pub(crate) struct Indices {
    current_x: usize,
    to_x: usize,

    current_y: usize,
    to_y: usize,

    z: usize,
}

impl From<Rectangle> for Indices {
    fn from(r: Rectangle) -> Indices {
        Indices {
            z: r.z(),
            current_x: r.x(),
            current_y: r.y(),
            to_x: r.x() + r.width(),
            to_y: r.y() + r.height(),
        }
    }
}

impl Iterator for Indices {
    type Item = Idx;
    fn next(&mut self) -> Option<Self::Item> {
        match (self.current_x, self.current_y) {
            (x, y) if (x == self.to_x && y == self.to_y) => None,
            (x, y) if (x == self.to_x && y < self.to_y) => {
                let idx = Idx(x, y, self.z);
                self.current_x = 0;
                self.current_y += 1;
                Some(idx)
            }
            (x, y) if (x < self.to_x) => {
                let idx = Idx(x, y, self.z);
                self.current_x += 1;
                Some(idx)
            }
            (_, _) => unreachable!(),
        }
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
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub(crate) fn opposite(&self) -> Direction {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
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

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    fn rectangle(x: usize, y: usize, z: usize, width: usize, height: usize) -> Rectangle {
        Rectangle(Idx(x, y, z), Bounds2D(width, height))
    }

    #[rstest]
    #[case::move_right(
        1,
        Direction::Right,
        rectangle(0, 0, 0, 5, 5,),
        rectangle(1, 0, 0, 5, 5,)
    )]
    #[case::move_left(
        1,
        Direction::Left,
        rectangle(10, 0, 0, 5, 5,),
        rectangle(9, 0, 0, 5, 5,)
    )]
    #[case::move_left_to_zero(
        1,
        Direction::Left,
        rectangle(1, 0, 0, 5, 5,),
        rectangle(0, 0, 0, 5, 5,)
    )]
    #[case::move_left_already_at_zero(
        1,
        Direction::Left,
        rectangle(0, 0, 0, 5, 5,),
        rectangle(0, 0, 0, 5, 5,)
    )]
    #[case::move_up(
        1,
        Direction::Up,
        rectangle(0, 10, 0, 5, 5,),
        rectangle(0, 9, 0, 5, 5,)
    )]
    #[case::move_up_to_zero(1, Direction::Up, rectangle(0, 1, 0, 5, 5,), rectangle(0, 0, 0, 5, 5,))]
    #[case::move_up_already_at_zero(
        1,
        Direction::Up,
        rectangle(0, 0, 0, 5, 5,),
        rectangle(0, 0, 0, 5, 5,)
    )]
    #[case::move_down(
        1,
        Direction::Down,
        rectangle(0, 0, 0, 5, 5,),
        rectangle(0, 1, 0, 5, 5,)
    )]
    fn rectangle_translate(
        #[case] magnitude: usize,
        #[case] direction: Direction,
        #[case] initial: Rectangle,
        #[case] expected: Rectangle,
    ) -> Result<()> {
        let mut updated = initial.clone();
        updated.translate(magnitude, direction)?;
        assert_eq!(expected, updated);
        Ok(())
    }
}
