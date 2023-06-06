use std::cell::RefCell;
use std::rc::Rc;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
};

use rand::rngs::ThreadRng;
use rand::{thread_rng, Rng};

/// The Result type for tui48.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Direction represents the direction indicated by the player.
#[derive(Clone, Default)]
enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

/// Board represents a 2048 board that keeps track of the history of its game states.
struct Board {
    rng: ThreadRng,
    rounds: Vec<Round>,
}

impl Board {
    fn new(mut rng: ThreadRng) -> Self {
        let mut rounds = Vec::with_capacity(2000);
        rounds.push(Round::random(&mut rng));
        Self { rng, rounds }
    }

    fn score(&self) -> u16 {
        self.rounds.last().map_or(0, |r| r.score)
    }

    /// try_shift attempts to shift the board in the given direction and returns an AnimationHint
    /// if anything changes.
    fn try_shift(&mut self, direction: Direction) -> Option<AnimationHint> {
        let prev = self
            .rounds
            .last()
            .expect("there should always be a previous round");
        let mut hint = AnimationHint::default();
        let round = Rc::new(RefCell::new(prev.clone()));
        let idxs = Round::iter_mut(round.clone(), direction).collect::<Vec<Idx>>();
        {
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
                    // if the pivot element is 0 and the cmp isn't, replace the pivot element with
                    // the cmp and zero the cmp
                    if pivot == 0 {
                        round.set(pivot_idx, cmp);
                        round.set(cmp_idx, 0);
                        hint.set(cmp_idx, pivot_idx.clone());
                        continue;
                    }
                    // if the pivot element and the cmp element are equal then they must be
                    // combined; do so and increment the score by the value of the eliminated
                    // element
                    if pivot == cmp {
                        round.score += cmp;
                        round.set(pivot_idx, pivot + cmp);
                        round.set(cmp_idx, 0);
                        hint.set(cmp_idx, pivot_idx.clone());
                    }
                    // at this point we can consider the current cmp element to be the new pivot
                    // for subsequent iterations
                    pivot_idx = cmp_idx;
                }
            }
        }

        if hint.changed {
            self.rounds.push(
                Rc::into_inner(round)
                    .expect("there should only be one strong reference at this point")
                    .into_inner(),
            );
            Some(hint)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct AnimationHint {
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
struct Round {
    slots: [[u16; 4]; 4],
    score: u16,
}

impl Round {
    fn random(rng: &mut ThreadRng) -> Self {
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

    fn iter_mut(round: Rc<RefCell<Round>>, direction: Direction) -> RoundIterator {
        RoundIterator {
            round,
            direction,
            xdx: 0,
            ydx: 0,
        }
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
}

struct RoundIterator {
    round: Rc<RefCell<Round>>,
    direction: Direction,
    xdx: usize,
    ydx: usize,
}

impl RoundIterator {
    fn new(round: Rc<RefCell<Round>>, direction: Direction) -> Self {
        let (xdx, ydx) = match direction {
            Direction::Left => (0, 0),
            Direction::Right => (3, 0),
            Direction::Up => (0, 0),
            Direction::Down => (0, 3),
        };

        RoundIterator {
            round,
            direction,
            xdx,
            ydx,
        }
    }
}

#[derive(Clone, Default)]
struct Idx(usize, usize);

impl Iterator for RoundIterator {
    type Item = Idx;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.xdx, self.ydx) == (3, 3) {
            return None;
        }
        match (&self.direction, self.xdx, self.ydx) {
            (Direction::Left, 3, 3) => None,
            (Direction::Left, xdx, ydx) => {
                if xdx == 3 {
                    self.xdx = 0;
                    self.ydx += 1;
                } else {
                    self.xdx += 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Right, 0, 3) => None,
            (Direction::Right, xdx, ydx) => {
                if xdx == 0 {
                    self.xdx = 0;
                    self.ydx += 1;
                } else {
                    self.xdx -= 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Up, 3, 3) => None,
            (Direction::Up, xdx, ydx) => {
                if ydx == 3 {
                    self.ydx = 0;
                    self.xdx += 1;
                } else {
                    self.ydx += 1;
                }
                Some(Idx(xdx, ydx))
            }
            (Direction::Down, 3, 0) => None,
            (Direction::Down, xdx, ydx) => {
                if ydx == 0 {
                    self.ydx = 3;
                    self.xdx += 1;
                } else {
                    self.ydx -= 1;
                }
                Some(Idx(xdx, ydx))
            }
        }
    }
}

fn main() -> Result<()> {
    let rng = thread_rng();
    let mut board = Board::new(rng);

    terminal::enable_raw_mode()?;
    while let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Enter => {
                break;
            }
            KeyCode::Char(c) => {
                break;
            }
            _ => {}
        }
    }
    terminal::disable_raw_mode()?;

    return Ok(());
}
