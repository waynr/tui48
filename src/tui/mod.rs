use std::io::Write;

use crate::board::{Board, Direction};
use crate::error::Result;

mod canvas;
use canvas::{Bounds2D, Canvas, Idx, Modifier, Rectangle};
mod crossterm;
use crate::tui::crossterm::{next_event, size, Crossterm};

pub(crate) trait Renderer {
    fn render(&mut self, c: &Canvas) -> Result<()>;
}

pub(crate) enum Event {
    UserInput(UserInput),
}

pub(crate) enum UserInput {
    Direction(Direction),
    Quit,
}

pub(crate) struct Tui48 {
    renderer: Box<dyn Renderer>,
    canvas: Canvas,
    board: Board,
}

impl Tui48 {
    pub(crate) fn new<T: Write + 'static>(board: Board, w: Box<T>) -> Result<Self> {
        let (width, height) = size()?;
        Ok(Self {
            board,
            renderer: Box::new(Crossterm::<T>::new(w)?),
            canvas: Canvas::new(width as usize, height as usize),
        })
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    pub(crate) fn run(mut self) -> Result<()> {
        let mut buf = self.canvas.get_layer(0)?;
        buf.modify_before(Modifier::ForegroundColor(0, 0, 0));
        buf.modify_before(Modifier::BackgroundColor(150, 150, 150));
        let mut small = self
            .canvas
            .get_draw_buffer(Rectangle(Idx(2, 5, 2), Bounds2D(20, 6)))?;
        small.draw_border()?;
        small.fill(' ')?;
        let mut overlapping_small = self
            .canvas
            .get_draw_buffer(Rectangle(Idx(10, 3, 3), Bounds2D(20, 30)))?;
        overlapping_small.draw_border()?;
        overlapping_small.fill('o')?;
        self.renderer.render(&self.canvas)?;
        //self.initialize_terminal()?;
        //self.draw_board()?;
        //self.w.flush()?;

        loop {
            match next_event()? {
                Event::UserInput(UserInput::Direction(d)) => self.shift(d)?,
                Event::UserInput(UserInput::Quit) => break,
            }
        }

        Ok(())
    }
}

impl Tui48 {
    fn shift(&mut self, direction: Direction) -> Result<()> {
        if let Some(hint) = self.board.shift(direction) {}
        Ok(())
    }
}
