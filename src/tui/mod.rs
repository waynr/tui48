use std::io::Write;

use crate::board::{Board, Direction};
use crate::error::Result;

mod canvas;
use canvas::{Canvas, Modifier};
mod crossterm;
use crate::tui::crossterm::{
    Crossterm, size, next_event,
};

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
