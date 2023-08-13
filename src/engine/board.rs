use rand::RngCore;

use super::round::{AnimationHint, Round, Score};
use crate::tui::geometry::Direction;

/// Board represents a 2048 board that keeps track of the history of its game states.
pub(crate) struct Board {
    rng: Box<dyn RngCore>,
    rounds: Vec<Round>,
}

impl Board {
    /// Initialize new board using the given random number generator.
    pub(crate) fn new(mut rng: impl RngCore + 'static) -> Self {
        let mut rounds = Vec::with_capacity(2000);
        rounds.push(Round::random(&mut rng));
        Self {
            rng: Box::new(rng),
            rounds,
        }
    }

    pub(crate) fn score(&self) -> Score {
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
        let hint = round.shift(&mut self.rng, &direction);

        if hint.is_some() {
            self.rounds.push(round);
        }
        hint
    }

    pub(crate) fn current(&self) -> Round {
        self.rounds
            .last()
            .expect("a board must always have at least one round")
            .clone()
    }

    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (4, 4)
    }

    pub(crate) fn is_game_over(&self) -> bool {
        self.rounds
            .last()
            .expect("a board must always have at least one round")
            .is_game_over(&Direction::Right)
    }

    #[cfg(test)]
    pub(crate) fn set_initial_round(&mut self, round: Round) {
        let mut v = Vec::with_capacity(1);
        v.push(round);
        self.rounds = v;
    }
}
