use rand::distributions::Distribution;
use rand::distributions::WeightedIndex;
use rand::seq::IteratorRandom;
use rand::Rng;

use crate::tui::geometry::Direction;

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) struct Idx(pub(crate) usize, pub(crate) usize);

impl std::fmt::Display for Idx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ridx({0},{1})", self.0, self.1)
    }
}

impl Idx {
    pub(crate) fn x(&self) -> usize {
        self.0
    }

    pub(crate) fn y(&self) -> usize {
        self.1
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum Hint {
    ToIdx(Idx),
    NewValueToIdx(u16, Idx),
    NewTile(u16, Direction),
}

impl std::fmt::Display for Hint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ToIdx(idx) => write!(f, "Hint::ToIdx({0})", idx),
            Self::NewValueToIdx(value, idx) => {
                write!(f, "Hint::NewValueToIdx({0}, {1})", value, idx)
            }
            Self::NewTile(value, direction) => {
                write!(f, "Hint::NewTile({0}, {1})", value, direction)
            }
        }
    }
}

#[derive(Default, PartialEq)]
pub(crate) struct AnimationHint {
    hint: Vec<(Idx, Hint)>,
    changed: bool,
}

impl std::fmt::Display for AnimationHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.hint.len() > 0 {
            write!(f, "\n")?;
        }
        for (idx, hint) in &self.hint {
            write!(f, "  {0} - {1}\n", idx, hint)?;
        }
        Ok(())
    }
}

impl AnimationHint {
    fn new() -> Self {
        Self {
            hint: Vec::new(),
            changed: false,
        }
    }

    fn set(&mut self, idx: &Idx, value: Hint) {
        self.changed = true;
        self.hint.push((idx.clone(), value));
    }

    pub(crate) fn hints(&self) -> Vec<(Idx, Hint)> {
        self.hint.clone()
    }

    pub(crate) fn remove(&mut self, idx: &Idx, hint: &Hint) {
        let mut remove_idx = 0usize;
        for (hint_idx, (i, h)) in self.hint.iter().enumerate() {
            if *i == *idx && *h == *hint {
                remove_idx = hint_idx;
            }
        }
        let _ = self.hint.remove(remove_idx);
    }
}

pub(crate) type Card = u16;

pub(crate) type Score = u16;

const NEW_CARD_CHOICES: [u16; 2] = [2, 4];
const NEW_CARD_WEIGHTS: [u8; 2] = [9, 1];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Round {
    slots: [[Card; 4]; 4],
    score: Score,
    new_tile_weighted_index: WeightedIndex<u8>,
}

impl Default for Round {
    fn default() -> Self {
        Round {
            slots: [[0; 4]; 4],
            score: Score::default(),
            new_tile_weighted_index: WeightedIndex::new(NEW_CARD_WEIGHTS)
                .expect("NEW_CARD_WEIGHTS should never be empty"),
        }
    }
}

// public methods
impl Round {
    pub(crate) fn score(&self) -> Score {
        self.score
    }

    pub(crate) fn random<T: Rng>(rng: &mut T) -> Self {
        let mut r = Round::default();
        let (xdx1, ydx1) = (rng.gen_range(0..3), rng.gen_range(0..3));
        let (xdx2, ydx2) = (rng.gen_range(0..3), rng.gen_range(0..3));
        loop {
            let (xdx2, ydx2) = (rng.gen_range(0..3), rng.gen_range(0..3));
            if (xdx1, ydx1) == (xdx2, ydx2) {
                continue;
            }
            break;
        }
        r.slots[ydx1][xdx1] = 2;
        r.slots[ydx2][xdx2] = 2;
        r
    }

    pub(crate) fn get(&self, idx: &Idx) -> Card {
        *self
            .slots
            .get(idx.1)
            .expect(format!("invalid y coordinate {}", idx.1).as_str())
            .get(idx.0)
            .expect(format!("invalid x coordinate {}", idx.0).as_str())
    }

    pub fn shift<T: Rng>(&mut self, mut rng: T, direction: &Direction) -> Option<AnimationHint> {
        let mut hint = AnimationHint::new();
        let idxs = self.iter_mut(direction.clone()).collect::<Vec<Idx>>();
        let rows = idxs.chunks(4);
        for row in rows {
            let mut pivot_iter = row.iter();
            let mut pivot_idx = pivot_iter.next().expect("should always yield an index");
            let mut cmp_iter = pivot_iter.clone();
            while let Some(cmp_idx) = cmp_iter.next() {
                let pivot = self.get(pivot_idx);
                let cmp = self.get(cmp_idx);
                // if the cmp element is 0, move on to the next element in the row
                if cmp == 0 {
                    continue;
                }
                // if the pivot element is 0 and the cmp isn't, replace the pivot element with the
                // cmp and zero the cmp
                if pivot == 0 {
                    self.set(pivot_idx, cmp);
                    self.set(cmp_idx, 0);
                    hint.set(cmp_idx, Hint::ToIdx(pivot_idx.clone()));
                    continue;
                }
                // if the pivot element and the cmp element are equal then they must be combined;
                // do so and increment the score by the value of the eliminated element
                if pivot == cmp {
                    let new_value = pivot + cmp;
                    self.score += cmp;
                    self.set(pivot_idx, pivot + cmp);
                    self.set(cmp_idx, 0);
                    hint.set(cmp_idx, Hint::NewValueToIdx(new_value, pivot_idx.clone()));
                }
                if let Some(idx) = pivot_iter.next() {
                    pivot_idx = idx;
                    cmp_iter = pivot_iter.clone();
                } else {
                    break; // no more pivots to test!
                }
            }
        }
        if hint.changed {
            let idx = idxs
                .chunks(4)
                .map(|row| row.last().expect("all rows are expected to be populated"))
                .filter(|idx| self.get(idx) == 0)
                .choose(&mut rng)
                .expect("all rows are populated and at least one row has changed");
            let new_value = NEW_CARD_CHOICES[self.new_tile_weighted_index.sample(&mut rng)];
            self.set(idx, new_value);
            hint.set(idx, Hint::NewTile(new_value, direction.clone()));
            Some(hint)
        } else {
            None
        }
    }
}

// private methods
impl Round {
    fn iter_mut(&self, direction: Direction) -> Indices {
        Indices::new(self, direction)
    }

    fn get_mut(&mut self, idx: &Idx) -> &mut Card {
        self.slots
            .get_mut(idx.1)
            .expect(format!("invalid y coordinate {}", idx.1).as_str())
            .get_mut(idx.0)
            .expect(format!("invalid x coordinate {}", idx.0).as_str())
    }

    fn set(&mut self, idx: &Idx, value: Card) {
        let rf = self.get_mut(idx);
        *rf = value;
    }

    pub(crate) fn set_value(&mut self, idx: &Idx, value: u16) {
        let rf = self.get_mut(idx);
        *rf = value;
    }
}

// Indices is an iterator of Idx over a given round's 2d array of slots.
struct Indices {
    direction: Direction,
    x_width: usize,
    y_width: usize,
    xdx: usize,
    ydx: usize,
}

impl Indices {
    fn new(round: &Round, direction: Direction) -> Self {
        let (x_width, y_width) = { (round.slots.len(), round.slots[0].len()) };

        let (xdx, ydx) = match direction {
            Direction::Left => (0, 0),
            Direction::Right => (x_width - 1, 0),
            Direction::Up => (0, 0),
            Direction::Down => (0, y_width - 1),
        };

        Indices {
            direction,
            x_width,
            y_width,
            xdx,
            ydx,
        }
    }
}

impl Iterator for Indices {
    type Item = Idx;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.direction {
            Direction::Left => self.next_left(),
            Direction::Right => self.next_right(),
            Direction::Up => self.next_up(),
            Direction::Down => self.next_down(),
        }
    }
}

impl Indices {
    fn next_left(&mut self) -> Option<Idx> {
        let (xdx, ydx) = (self.xdx, self.ydx);
        if ydx == self.y_width {
            return None;
        }
        if xdx == self.x_width - 1 {
            self.xdx = 0;
            self.ydx += 1;
        } else {
            self.xdx += 1;
        }
        Some(Idx(xdx, ydx))
    }
    fn next_right(&mut self) -> Option<Idx> {
        let (xdx, ydx) = (self.xdx, self.ydx);
        if ydx == self.y_width {
            return None;
        }
        if xdx == 0 {
            self.xdx = self.x_width - 1;
            self.ydx += 1;
        } else {
            self.xdx -= 1;
        }
        Some(Idx(xdx, ydx))
    }
    fn next_up(&mut self) -> Option<Idx> {
        let (xdx, ydx) = (self.xdx, self.ydx);
        if xdx == self.x_width {
            return None;
        }
        if ydx == self.y_width - 1 {
            self.ydx = 0;
            self.xdx += 1;
        } else {
            self.ydx += 1;
        }
        Some(Idx(xdx, ydx))
    }
    fn next_down(&mut self) -> Option<Idx> {
        let (xdx, ydx) = (self.xdx, self.ydx);
        if xdx == self.x_width {
            return None;
        }
        if ydx == 0 {
            self.ydx = self.y_width - 1;
            self.xdx += 1;
        } else {
            self.ydx -= 1;
        }
        Some(Idx(xdx, ydx))
    }
}

#[cfg(test)]
mod test {
    use rand::rngs::SmallRng;
    use rand::SeedableRng;
    use rstest::*;

    use super::*;
    fn rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    fn round(slots: [[Card; 4]; 4], score: Score) -> Round {
        let mut r = Round::default();
        r.slots = slots;
        r.score = score;
        r
    }

    #[test]
    fn clone() {
        let initial = Round::default();
        let cloned = initial.clone();
        assert_eq!(initial, cloned);
        assert_eq!(initial.score, cloned.score);
    }

    #[test]
    fn shift_empty() {
        let initial = Round::default();

        for direction in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            let mut shifted = initial.clone();
            let mut rng = rng();
            let hint = shifted.shift(&mut rng, &direction);
            assert_eq!(initial, shifted, "shifting {:?}", direction);
            assert_eq!(initial.score, shifted.score, "shifting {:?}", direction);
        }
    }

    #[rstest]
    #[case::identity_left(Direction::Left,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[1, 0, 0, 2], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
    )]
    #[case::identity_right(Direction::Right,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[2, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
    )]
    #[case::identity_up(Direction::Up,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [2, 0, 0, 0]],
    )]
    #[case::identity_down(Direction::Down,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[2, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
    )]
    #[case::flipped_identity_left(Direction::Left,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[1, 0, 0, 2], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
    )]
    #[case::flipped_identity_right(Direction::Right,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[2, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
    )]
    #[case::flipped_identity_up(Direction::Up,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [2, 0, 0, 0]],
    )]
    #[case::flipped_identity_down(Direction::Down,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[2, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
    )]
    #[case::all_left(Direction::Left,
           [[0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
           [[1, 0, 0, 2], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
    )]
    #[case::all_right(Direction::Right,
           [[1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
           [[2, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
    )]
    #[case::all_down(Direction::Down,
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
           [[2, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
    )]
    #[case::all_up(Direction::Up,
           [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [2, 0, 0, 0]],
    )]
    #[case::pivot_is_zero_with_multiple_shift_elements(Direction::Left,
           [[0, 1, 2, 3], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
           [[1, 2, 3, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
    )]
    fn shift(
        #[case] direction: Direction,
        #[case] initial: [[Card; 4]; 4],
        #[case] expected: [[Card; 4]; 4],
    ) {
        let initial = round(initial, 0);
        let expected = round(expected, 0);

        let mut shifted = initial.clone();
        let mut rng = rng();
        let hint = shifted.shift(&mut rng, &direction);
        assert_eq!(shifted, expected, "shifting {:?}", direction);
    }

    #[rstest]
    #[case::all1s(
        Direction::Left,
        round([[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 0),
        round([[2, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 2),
    )]
    #[case::combine2s_shift_remaining(
        Direction::Left,
        round([[2, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 2),
        round([[4, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
    )]
    #[case::combine2s_shift_remaining(
        Direction::Left,
        round([[2, 0, 2, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 2),
        round([[4, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
    )]
    #[case::combine2s_ignore_4(
        Direction::Left,
        round([[4, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
        round([[4, 4, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 6),
    )]
    #[case::noop_no_compatible_combinations(
        Direction::Left,
        round([[2, 4, 8, 16], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
        round([[2, 4, 8, 16], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
    )]
    #[case::all1s_right(
        Direction::Right,
        round([[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 0),
        round([[2, 0, 2, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 2),
    )]
    #[case::combine2s_shift_remaining_right(
        Direction::Right,
        round([[2, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 2),
        round([[2, 0, 2, 4], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
    )]
    #[case::combine2s_ignore_4_right(
        Direction::Right,
        round([[4, 2, 0, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
        round([[2, 0, 4, 4], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 6),
    )]
    #[case::noop_no_compatible_combinations_right(
        Direction::Right,
        round([[2, 4, 8, 16], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
        round([[2, 4, 8, 16], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]], 4),
    )]
    fn combine(#[case] direction: Direction, #[case] initial: Round, #[case] expected: Round) {
        let mut shifted = initial.clone();
        let mut rng = rng();
        let hint = shifted.shift(&mut rng, &direction);
        assert_eq!(shifted, expected, "shifting {:?}", direction);
    }
}
