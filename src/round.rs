use std::cell::RefCell;
use std::rc::Rc;

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
            .expect("Idx should never be invalid")
            .get_mut(idx.0)
            .expect("Idx should never be invalid")
    }

    fn set(&mut self, idx: &Idx, value: Idx) {
        self.changed = true;
        let rf = self.get_mut(idx);
        *rf = Some(value);
    }
}

#[derive(Clone, Default)]
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
    fn iter_mut(round: Rc<RefCell<Round>>, direction: Direction) -> RoundIterator {
        RoundIterator::new(round, direction)
    }

    fn get(&self, idx: &Idx) -> u16 {
        *self
            .slots
            .get(idx.1)
            .expect("Idx should never be invalid")
            .get(idx.0)
            .expect("Idx should never be invalid")
    }

    fn get_mut(&mut self, idx: &Idx) -> &mut u16 {
        self.slots
            .get_mut(idx.1)
            .expect("Idx should never be invalid")
            .get_mut(idx.0)
            .expect("Idx should never be invalid")
    }

    fn set(&mut self, idx: &Idx, value: u16) {
        let rf = self.get_mut(idx);
        *rf = value;
    }

    pub fn shift(round: Rc<RefCell<Self>>, direction: Direction) -> Option<AnimationHint> {
        let mut hint = AnimationHint::default();
        let idxs = Round::iter_mut(round.clone(), direction).collect::<Vec<Idx>>();
        let mut round = round.borrow_mut();
        let rows = idxs.chunks(4);
        for row in rows {
            let mut row_iter = row.iter();
            let mut pivot_idx = row_iter.next().expect("should always yield an index");
            let mut empty_slot_idx: Option<Idx> = None;
            while let Some(cmp_idx) = row_iter.next() {
                let pivot = round.get(pivot_idx);
                let cmp = round.get(cmp_idx);
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
                    round.set(pivot_idx, cmp);
                    round.set(cmp_idx, 0);
                    hint.set(cmp_idx, pivot_idx.clone());
                    continue;
                }
                // if the pivot element and the cmp element are equal then they must be combined;
                // do so and increment the score by the value of the eliminated element
                if pivot == cmp {
                    round.score += cmp;
                    round.set(pivot_idx, pivot + cmp);
                    round.set(cmp_idx, 0);
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
    max_xdx: usize,
    max_ydx: usize,
    xdx: usize,
    ydx: usize,
}

impl RoundIterator {
    fn new(round: Rc<RefCell<Round>>, direction: Direction) -> Self {
        let (max_xdx, max_ydx) = {
            let round = round.borrow();
            (round.slots.len() - 1, round.slots[0].len() - 1)
        };

        let (xdx, ydx) = match direction {
            Direction::Left => (0, 0),
            Direction::Right => (max_xdx, 0),
            Direction::Up => (0, 0),
            Direction::Down => (0, max_ydx),
        };

        RoundIterator {
            direction,
            max_xdx,
            max_ydx,
            xdx,
            ydx,
        }
    }
}

impl Iterator for RoundIterator {
    type Item = Idx;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.xdx, self.ydx) == (self.max_xdx, self.max_ydx) {
            return None;
        }
        match (&self.direction, self.xdx, self.ydx) {
            (Direction::Left, xdx, ydx) if (xdx, ydx) == (self.max_xdx, self.max_ydx) => None,
            (Direction::Left, xdx, ydx) => {
                if xdx == self.max_xdx {
                    self.xdx = 0;
                    self.ydx += 1;
                } else {
                    self.xdx += 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Right, 0, ydx) if ydx == self.max_ydx => None,
            (Direction::Right, xdx, ydx) => {
                if xdx == 0 {
                    self.xdx = self.max_xdx;
                    self.ydx += 1;
                } else {
                    self.xdx -= 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Up, xdx, ydx) if (xdx, ydx) == (self.max_xdx, self.max_ydx) => None,
            (Direction::Up, xdx, ydx) => {
                if ydx == self.max_ydx {
                    self.ydx = 0;
                    self.xdx += 1;
                } else {
                    self.ydx += 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Down, xdx, 0) if xdx == self.max_xdx => None,
            (Direction::Down, xdx, ydx) => {
                if ydx == 0 {
                    self.ydx = self.max_ydx;
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
    #[test]
    fn round_shift_left() {}
}
