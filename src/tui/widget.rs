use std::io::Write;

use crate::board::Board;
use crate::error::Result;
use crate::round::{Card, Round};

pub(crate) struct Bounds(u16, u16);

pub(crate) enum SizeHint {
    Unknown,
    MinBounds(Bounds),
}

pub(crate) trait Widget<W: Write> {
    /// Using the given w, draw the widget within the given bounds. Note that this method should
    /// assume a parent widget has already set the initial position of the widget and that all
    /// cursor movement should be relative to that starting position. Absolute cursor positioning
    /// here will likely corrupt the output buffer.
    fn draw(&self, w: W, b: Bounds) -> Result<Bounds>;

    /// Return a `SizeHint` to let the parent widget know the preferred or minimum size of the
    /// child.
    fn size_hint(&self) -> SizeHint {
        SizeHint::Unknown
    }
}

impl<W: Write> Widget<W> for Board {
    fn draw(&self, w: W, b: Bounds) -> Result<Bounds> {
        Ok(Bounds(0,0))
    }
}

impl<W: Write> Widget<W> for Round {
    fn draw(&self, w: W, b: Bounds) -> Result<Bounds> {
        Ok(Bounds(0,0))
    }
}

impl<W: Write> Widget<W> for Card {
    fn draw(&self, w: W, b: Bounds) -> Result<Bounds> {
        Ok(Bounds(0,0))
    }
}
