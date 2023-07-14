use super::error::{InnerError, Result};

/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Idx(pub usize, pub usize, pub usize);

impl std::fmt::Display for Idx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "idx({0},{1},{2})", self.0, self.1, self.2)
    }
}

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

impl std::fmt::Display for Bounds2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dims({0}, {1})", self.0, self.1)
    }
}

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

impl std::fmt::Display for Rectangle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rect({0} {1})", self.0, self.1,)
    }
}

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
            Position::Coordinates(x, y) => (*x, *y),
            Position::Idx(Idx(x, y, _z)) => (*x, *y),
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
                return Err(InnerError::InvalidVectorTranslation {
                    mag,
                    dir: dir.clone(),
                    rect: self.clone(),
                }
                .into())
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn extents(&self) -> (usize, usize) {
        (self.0 .0 + self.1 .0, self.0 .1 + self.1 .1)
    }

    #[inline(always)]
    pub(crate) fn contains_or_err(&self, geo: Geometry) -> Result<()> {
        match geo {
            Geometry::Idx(idx) => {
                if idx.x() < self.x() || idx.x() > self.x() + self.width() {
                    return Err(InnerError::OutOfBoundsX(idx.x()).into());
                }
                if idx.y() < self.y() || idx.y() > self.y() + self.height() {
                    return Err(InnerError::OutOfBoundsY(idx.y()).into());
                }
                Ok(())
            }
            Geometry::Rectangle(rect) => {
                let (x_extent, y_extent) = rect.extents();
                if x_extent > self.width() {
                    return Err(InnerError::OutOfBoundsX(x_extent).into());
                }
                if y_extent > self.height() {
                    return Err(InnerError::OutOfBoundsY(y_extent).into());
                }
                Ok(())
            }
        }
    }
}

pub(crate) enum Geometry<'a> {
    Idx(&'a Idx),
    Rectangle(&'a Rectangle),
}

pub(crate) struct Indices {
    from_x: usize,
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
            from_x: r.x(),
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
        if self.current_x == self.to_x && self.current_y == self.to_y {
            None
        } else if self.current_x == self.to_x && self.current_y < self.to_y {
            let idx = Idx(self.current_x, self.current_y, self.z);
            self.current_x = self.from_x;
            self.current_y += 1;
            Some(idx)
        } else if self.current_x < self.to_x {
            let idx = Idx(self.current_x, self.current_y, self.z);
            self.current_x += 1;
            Some(idx)
        } else {
            unreachable!();
        }
    }
}

pub(crate) enum Position {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Coordinates(usize, usize),
    Idx(Idx),
}

impl From<Idx> for Position {
    fn from(idx: Idx) -> Self {
        Self::Idx(idx)
    }
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
        updated.translate(magnitude, &direction)?;
        assert_eq!(expected, updated);
        Ok(())
    }
}
