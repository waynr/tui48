use super::error::Result;

pub(crate) trait EventSource {
    fn next_event(&self) -> Result<Event>;
}

pub(crate) enum Event {
    UserInput(UserInput),
    Resize,
}

pub(crate) enum UserInput {
    Direction(Direction),
    Quit,
}

/// Direction represents the direction indicated by the player input.
pub(crate) enum Direction {
    Left,
    Right,
    Up,
    Down,
}
