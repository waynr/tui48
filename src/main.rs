use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
};

use rand::rngs::ThreadRng;
use rand::{thread_rng, Rng};

/// The Result type for tui48.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Direction represents the direction indicated by the player.
#[derive(Default)]
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

    fn score(&self) -> u32 {
        self.rounds.last().map_or(0, |r| r.score)
    }

    fn shift(&mut self, direction: Direction) -> Option<AnimationHint> {
        let prev = self
            .rounds
            .last()
            .expect("there should always be a previous round");
        let mut next = prev.clone();
        {
            let _round_iter = Round::iter_mut(&mut next, direction);
        }
        self.rounds.push(next);
        Some(AnimationHint::default())
    }
}

#[derive(Default)]
struct AnimationHint {
    direction: Direction,
    hint: [[u8; 4]; 4],
}

#[derive(Clone, Default)]
struct Round {
    slots: [[u16; 4]; 4],
    score: u32,
}

impl<'a> Round {
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

    fn iter_mut(round: &'a mut Round, direction: Direction) -> RoundIterator<'a> {
        RoundIterator {
            round,
            direction,
            xdx: 0,
            ydx: 0,
        }
    }

    fn get(&mut self, xdx: usize, ydx: usize) -> Option<&mut u16> {
        match self.slots.get_mut(ydx) {
            Some(row) => row.get_mut(xdx),
            None => None,
        }
    }
}

struct RoundIterator<'a> {
    round: &'a mut Round,
    direction: Direction,
    xdx: usize,
    ydx: usize,
}

impl<'a> Iterator for RoundIterator<'a> {
    type Item = &'a mut u16;

    fn next(&mut self) -> Option<Self::Item> {
        if (self.xdx, self.ydx) == (3, 3) {
            return None;
        }
        let (xdx, ydx) = match self.direction {
            Direction::Left => (0, 0),
            Direction::Right => (0, 0),
            Direction::Up => (0, 0),
            Direction::Down => (0, 0),
        };
        //Some(
        //    self.round
        //        .get_mut(xdx, ydx)
        //        .expect("x and y indices must always be valid"),
        //)
        match self.round.slots.get_mut(ydx) {
            Some(row) => row.get_mut(xdx),
            None => None,
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
