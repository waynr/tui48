use rand::rngs::ThreadRng;
use rand::Rng;

use crate::board::Direction;

#[derive(Clone, Default)]
struct Idx(usize, usize);

#[derive(Default)]
pub(crate) struct AnimationHint {
    hint: [[Option<Idx>; 4]; 4],
    changed: bool,
}

impl AnimationHint {
    fn get_mut(&mut self, idx: &Idx) -> &mut Option<Idx> {
        self.hint
            .get_mut(idx.1)
            .expect(format!("invalid y coordinate {}", idx.1).as_str())
            .get_mut(idx.0)
            .expect(format!("invalid x coordinate {}", idx.0).as_str())
    }

    fn set(&mut self, idx: &Idx, value: Idx) {
        self.changed = true;
        let rf = self.get_mut(idx);
        *rf = Some(value);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Round {
    slots: [[u16; 4]; 4],
    score: u16,
}

// public methods
impl Round {
    pub(crate) fn score(&self) -> u16 {
        self.score
    }

    pub(crate) fn random(rng: &mut ThreadRng) -> Self {
        let mut r = Round::default();
        let (xdx2, ydx2) = (0, 0);
        let (xdx1, ydx1) = (rng.gen_range(0..3), rng.gen_range(0..3));
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
}

// private methods
impl Round {
    fn iter_mut(&self, direction: Direction) -> RoundIterator {
        RoundIterator::new(self, direction)
    }

    fn get(&self, idx: &Idx) -> u16 {
        *self
            .slots
            .get(idx.1)
            .expect(format!("invalid y coordinate {}", idx.1).as_str())
            .get(idx.0)
            .expect(format!("invalid x coordinate {}", idx.0).as_str())
    }

    fn get_mut(&mut self, idx: &Idx) -> &mut u16 {
        self.slots
            .get_mut(idx.1)
            .expect(format!("invalid y coordinate {}", idx.1).as_str())
            .get_mut(idx.0)
            .expect(format!("invalid x coordinate {}", idx.0).as_str())
    }

    fn set(&mut self, idx: &Idx, value: u16) {
        let rf = self.get_mut(idx);
        *rf = value;
    }

    pub fn shift(&mut self, direction: &Direction) -> Option<AnimationHint> {
        let mut hint = AnimationHint::default();
        let idxs = self.iter_mut(direction.clone()).collect::<Vec<Idx>>();
        let rows = idxs.chunks(4);
        for row in rows {
            let mut row_iter = row.iter();
            let mut pivot_idx = row_iter.next().expect("should always yield an index");
            let mut empty_slot_idx: Option<Idx> = None;
            while let Some(cmp_idx) = row_iter.next() {
                let pivot = self.get(pivot_idx);
                let cmp = self.get(cmp_idx);
                // if the cmp element is 0, move on to the next element in the row
                if cmp == 0 {
                    if empty_slot_idx.is_none() {
                        empty_slot_idx = Some(cmp_idx.clone());
                    }
                    continue;
                }
                // if the pivot element is 0 and the cmp isn't, replace the pivot element with the
                // cmp and zero the cmp
                if pivot == 0 {
                    self.set(pivot_idx, cmp);
                    self.set(cmp_idx, 0);
                    hint.set(cmp_idx, pivot_idx.clone());
                    continue;
                }
                // if the pivot element and the cmp element are equal then they must be combined;
                // do so and increment the score by the value of the eliminated element
                if pivot == cmp {
                    self.score += cmp;
                    self.set(pivot_idx, pivot + cmp);
                    self.set(cmp_idx, 0);
                    hint.set(cmp_idx, pivot_idx.clone());
                }
                // at this point we can consider the current cmp element to be the new pivot for
                // subsequent iterations
                pivot_idx = cmp_idx;
            }
        }
        if hint.changed {
            Some(hint)
        } else {
            None
        }
    }
}

struct RoundIterator {
    direction: Direction,
    x_width: usize,
    y_width: usize,
    xdx: usize,
    ydx: usize,
}

impl RoundIterator {
    fn new(round: &Round, direction: Direction) -> Self {
        let (x_width, y_width) = { (round.slots.len(), round.slots[0].len()) };

        let (xdx, ydx) = match direction {
            Direction::Left => (0, 0),
            Direction::Right => (x_width-1, 0),
            Direction::Up => (0, 0),
            Direction::Down => (0, y_width-1),
        };

        RoundIterator {
            direction,
            x_width,
            y_width,
            xdx,
            ydx,
        }
    }
}

impl Iterator for RoundIterator {
    type Item = Idx;

    fn next(&mut self) -> Option<Self::Item> {
        match (&self.direction, self.xdx, self.ydx) {
            (Direction::Left, _, ydx) if ydx == self.y_width => None,
            (Direction::Left, xdx, ydx) => {
                if xdx == self.x_width - 1 {
                    self.xdx = 0;
                    self.ydx += 1;
                } else {
                    self.xdx += 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Right, _, ydx) if ydx == self.y_width => None,
            (Direction::Right, xdx, ydx) => {
                if xdx == 0 {
                    self.xdx = self.x_width - 1;
                    self.ydx += 1;
                } else {
                    self.xdx -= 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Up, xdx, _) if xdx == self.x_width => None,
            (Direction::Up, xdx, ydx) => {
                if ydx == self.y_width - 1 {
                    self.ydx = 0;
                    self.xdx += 1;
                } else {
                    self.ydx += 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Down, xdx, _) if xdx == self.x_width => None,
            (Direction::Down, xdx, ydx) => {
                if ydx == 0 {
                    self.ydx = self.y_width - 1;
                    self.xdx += 1;
                } else {
                    self.ydx -= 1;
                }
                Some(Idx(xdx, ydx))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    #[test]
    fn clone() {
        let initial = Round {
            score: 0,
            slots: [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
        };
        let cloned = initial.clone();
        assert_eq!(initial, cloned);
        assert_eq!(initial.score, cloned.score);
    }

    #[test]
    fn shift_empty() {
        let initial = Round {
            score: 0,
            slots: [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
        };

        for direction in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            let mut shifted = initial.clone();
            let hint = shifted.shift(&direction);
            assert_eq!(initial, shifted, "shifting {:?}", direction);
            assert_eq!(initial.score, shifted.score, "shifting {:?}", direction);
        }
    }

    #[rstest]
    #[case::identity_left(Direction::Left, 
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
    )]
    #[case::identity_right(Direction::Right,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
    )]
    #[case::identity_up(Direction::Up,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
    )]
    #[case::identity_down(Direction::Down,
           [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]],
           [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
    )]
    #[case::flipped_identity_left(Direction::Left, 
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
    )]
    #[case::flipped_identity_right(Direction::Right,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
    )]
    #[case::flipped_identity_up(Direction::Up,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
    )]
    #[case::flipped_identity_down(Direction::Down,
           [[0, 0, 0, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 0, 0, 0]],
           [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
    )]
    #[case::all_left(Direction::Left,
           [[0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
           [[1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
    )]
    #[case::all_right(Direction::Right,
           [[1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]],
           [[0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
    )]
    #[case::all_down(Direction::Down,
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
           [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
    )]
    #[case::all_up(Direction::Up,
           [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
           [[1, 1, 1, 1], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]],
    )]
    fn shift(#[case] direction: Direction, #[case] initial: [[u16; 4]; 4], #[case] expected: [[u16; 4]; 4]) {
        let initial = Round {
            score: 0,
            slots: initial,
        };

        let expected = Round {
            score: 0,
            slots: expected,
        };

        let mut shifted = initial.clone();
        let hint = shifted.shift(&direction);
        assert_eq!(
            shifted, expected,
            "shifting {:?}",
            direction
        );
    }
}
