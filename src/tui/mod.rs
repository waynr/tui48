use std::io::Write;

use crate::board::{Board, Direction};
use crate::round::Idx as BoardIdx;
use crate::error::Result;

mod canvas;
use canvas::{Bounds2D, Canvas, DrawBuffer, Idx, Modifier, Rectangle};
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

struct Tui48Board {
    board: DrawBuffer,
    score: DrawBuffer,
    slots: Vec<Vec<Option<DrawBuffer>>>,
}

/// Generates a 2048 TUI layout with legible numbers.
///
///  37
///  ╔══════════════════════════════════╗
///  ║                                  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║                                  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║                                  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║                                  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║  xxxxxx  xxxxxx  xxxxxx  xxxxxx  ║
///  ║                                  ║
///  ╚══════════════════════════════════╝
///  65
///  6                                 42
///
///
///
impl Tui48Board {
    fn new(game: &Board, canvas: &mut Canvas) -> Result<Self> {
        let mut board = canvas.get_draw_buffer(Rectangle(Idx(5, 5, 0), Bounds2D(36, 25)))?;
        board.draw_border()?;
        board.fill(' ')?;

        let mut score = canvas.get_draw_buffer(Rectangle(Idx(18, 1, 0), Bounds2D(10, 3)))?;
        score.draw_border()?;
        score.fill(' ')?;
        score.write(&format!("{}", game.score()))?;

        let (width, height) = game.dimensions();
        let round = game.current();
        let mut slots = Vec::with_capacity(height);
        let x_offset = 5 + 1 + 2;
        let y_offset = 5 + 1  ;
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let mut opt = None;
                let value = round.get(&BoardIdx(x, y));
                if value > 0 {
                    let idx = Idx(x_offset + (2 + 6) * x, y_offset + (1 + 5) * y, 5);
                    let bounds = Bounds2D(6, 5);
                    let mut card_buffer = canvas.get_draw_buffer(Rectangle(idx, bounds))?;
                    card_buffer.draw_border()?;
                    card_buffer.fill(' ')?;
                    card_buffer.write(&format!("2048"))?;
                    opt = Some(card_buffer);
                }
                row.push(opt);
            }
            slots.push(row);
        }
        Ok(Self {
            board,
            score,
            slots,
        })
    }
}

pub(crate) struct Tui48 {
    renderer: Box<dyn Renderer>,
    canvas: Canvas,
    board: Board,
    tui_board: Tui48Board,
}

impl Tui48 {
    pub(crate) fn new<T: Write + 'static>(board: Board, w: Box<T>) -> Result<Self> {
        let (width, height) = size()?;
        let mut canvas = Canvas::new(width as usize, height as usize);
        let tui_board = Tui48Board::new(&board, &mut canvas)?;
        Ok(Self {
            board,
            renderer: Box::new(Crossterm::<T>::new(w)?),
            canvas,
            tui_board,
        })
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    pub(crate) fn run(mut self) -> Result<()> {
        self.renderer.render(&self.canvas)?;

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
