use super::error::{InnerError, Result};

/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Debug, Default, Eq, Ord, PartialOrd, PartialEq)]
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

    #[cfg(test)]
    #[inline(always)]
    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.width(), self.height())
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

impl IntoIterator for Rectangle {
    type Item = Idx;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut indices = Vec::new();
        if self.width() == 0 || self.height() == 0 {
            return indices.into_iter()
        }
        for x in self.x()..(self.x() + self.width()) {
            for y in self.y()..(self.y() + self.height()) {
                indices.push(Idx(x, y, self.z()));
            }
        }
        indices.into_iter()
    }
}

impl std::ops::Add for &Rectangle {
    type Output = Rectangle;
    fn add(self, other: &Rectangle) -> Self::Output {
        Rectangle(
            Idx(
                other.0.0,
                other.0.1,
                other.0.2,
            ),
            Bounds2D(
                self.1.0 + other.1.0,
                self.1.1 + other.1.1,
            )
        )
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
    use std::collections::BTreeSet;

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

    #[rstest]
    #[case::zero(rectangle(0, 0, 0, 0, 0), BTreeSet::new())]
    #[case::zerowidth(rectangle(0, 0, 0, 0, 1), BTreeSet::new())]
    #[case::zeroheight(rectangle(0, 0, 0, 1, 0), BTreeSet::new())]
    #[case::onebyone(
        rectangle(0, 0, 0, 1, 1),
        BTreeSet::from([Idx(0,0,0)]),
    )]
    #[case::twobytwo(
        rectangle(0, 0, 0, 2, 2),
        BTreeSet::from([Idx(0,0,0), Idx(0,1,0), Idx(1,0,0), Idx(1,1,0)]),
    )]
    #[case::threebyfive(
        rectangle(0, 0, 0, 3, 5),
        BTreeSet::from([
            Idx(0,0,0), Idx(1,0,0), Idx(2,0,0),
            Idx(0,1,0), Idx(1,1,0), Idx(2,1,0),
            Idx(0,2,0), Idx(1,2,0), Idx(2,2,0),
            Idx(0,3,0), Idx(1,3,0), Idx(2,3,0),
            Idx(0,4,0), Idx(1,4,0), Idx(2,4,0),
        ]),
    )]
    #[case::nonorigin(
        rectangle(999, 999, 0, 2, 2),
        BTreeSet::from([Idx(999,999,0), Idx(999,1000,0), Idx(1000,999,0), Idx(1000,1000,0)]),
    )]
    fn rectangle_to_indices(
        #[case] rectangle: Rectangle,
        #[case] expected_indices: BTreeSet<Idx>,
    ) -> Result<()> {
        let mut actual_indices: BTreeSet<Idx> = BTreeSet::new();

        for idx in rectangle.into_iter() {
            actual_indices.insert(idx);
        }

        // use set logic to verify actual and expected indices are correct
        let only_in_actual = actual_indices
            .difference(&expected_indices)
            .collect::<BTreeSet<_>>();
        let only_in_expected = expected_indices
            .difference(&actual_indices)
            .collect::<BTreeSet<_>>();
        assert!(
            only_in_actual.len() == 0,
            "\nFAILED BECAUSE - missing changed indices in the expected set:\n{:?}",
            &only_in_actual
        );
        assert!(
            only_in_expected.len() == 0,
            "\nFAILED BECAUSE - missing changed indices in the actual set:\n{:?}",
            &only_in_expected
        );

        Ok(())
    }
}
