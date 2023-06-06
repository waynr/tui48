use rand::rngs::ThreadRng;

use crate::round::{AnimationHint, Round};

/// Direction represents the direction indicated by the player.
#[derive(Clone, Default)]
pub(crate) enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

/// Board represents a 2048 board that keeps track of the history of its game states.
pub(crate) struct Board {
    rng: ThreadRng,
    rounds: Vec<Round>,
}

impl Board {
    /// Initialize new board using the given random number generator.
    pub(crate) fn new(mut rng: ThreadRng) -> Self {
        let mut rounds = Vec::with_capacity(2000);
        rounds.push(Round::random(&mut rng));
        Self { rng, rounds }
    }

    fn score(&self) -> u16 {
        self.rounds.last().map_or(0, |r| r.score())
    }

    /// try_shift attempts to shift the board in the given direction and returns an AnimationHint
    /// if anything changes.
    pub(crate) fn shift(&mut self, direction: Direction) -> Option<AnimationHint> {
        let prev = self
            .rounds
            .last()
            .expect("there should always be a previous round");
        let mut round = prev.clone();
        let hint = round.shift(direction.clone());

        if hint.is_some() {
            self.rounds.push(round);
        }
        hint
    }
}
