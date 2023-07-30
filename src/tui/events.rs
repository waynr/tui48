use super::error::Result;
use super::geometry::Direction;

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
