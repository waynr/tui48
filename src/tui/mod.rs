pub(crate) mod canvas;
pub(crate) mod drawbuffer;
pub(crate) mod geometry;
pub(crate) mod tuxel;
use canvas::{Canvas, Modifier};
use drawbuffer::DrawBuffer;
pub(crate) mod crossterm;
pub(crate) mod events;
pub(crate) mod renderer;

use crate::engine::board::{Board, Direction as GameDirection};
use crate::error::{Error, Result};
use crate::engine::round::Idx as BoardIdx;
use crate::tui::events::{Direction, Event, EventSource, UserInput};
use crate::tui::geometry::{Bounds2D, Idx, Rectangle};
use crate::tui::renderer::Renderer;

struct Tui48Board {
    _board: DrawBuffer,
    _score: DrawBuffer,
    _slots: Vec<Vec<Option<DrawBuffer>>>,
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
        let board_rectangle = Rectangle(Idx(5, 5, 0), Bounds2D(36, 25));
        let (cwidth, cheight) = canvas.dimensions();
        let (x_extent, y_extent) = board_rectangle.extents();
        if cwidth < x_extent || cheight < y_extent {
            return Err(Error::TerminalTooSmall(cwidth, cheight));
        }

        let mut board = canvas.get_draw_buffer(board_rectangle)?;
        board.draw_border()?;
        board.fill(' ')?;

        let mut score = canvas.get_draw_buffer(Rectangle(Idx(18, 1, 0), Bounds2D(10, 3)))?;
        score.draw_border()?;
        score.fill(' ')?;
        score.write_right(&format!("{}", game.score()))?;
        score.modify(Modifier::BackgroundColor(255, 255, 255));
        score.modify(Modifier::ForegroundColor(0, 0, 0));

        let (width, height) = game.dimensions();
        let round = game.current();
        let mut slots = Vec::with_capacity(height);
        let x_offset = 5 + 1 + 2;
        let y_offset = 5 + 1;
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
                    card_buffer.write_center(&format!("{}", value))?;
                    opt = Some(card_buffer);
                }
                row.push(opt);
            }
            slots.push(row);
        }
        Ok(Self {
            _board: board,
            _score: score,
            _slots: slots,
        })
    }
}

pub(crate) struct Tui48<R: Renderer, E: EventSource> {
    renderer: R,
    event_source: E,
    canvas: Canvas,
    board: Board,
    tui_board: Option<Tui48Board>,
}

impl<R: Renderer, E: EventSource> Tui48<R, E> {
    pub(crate) fn new(board: Board, renderer: R, event_source: E) -> Result<Self> {
        let (width, height) = renderer.size_hint()?;
        Ok(Self {
            board,
            renderer,
            event_source,
            canvas: Canvas::new(width as usize, height as usize),
            tui_board: None,
        })
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    pub(crate) fn run(mut self) -> Result<()> {
        self.resize()?;

        loop {
            let mut message_buf = match self.tui_board {
                Some(_) => None,
                None => {
                    let mut buf = self.canvas.get_layer(7)?;
                    buf.write_left("hey there! something is wrong! try resizing your terminal!")?;
                    Some(buf)
                }
            };

            self.renderer.render(&self.canvas)?;

            match self.event_source.next_event()? {
                Event::UserInput(UserInput::Direction(d)) => self.shift(d)?,
                Event::UserInput(UserInput::Quit) => break,
                Event::Resize => {
                    self.resize()?;
                    match message_buf.take() {
                        Some(b) => drop(b),
                        None => (),
                    };
                    self.renderer.clear(&self.canvas)?;
                }
            }
            self.canvas.draw_all()?;
        }
        Ok(())
    }
}

fn direction_to_game_direction(input: Direction) -> GameDirection {
    match input {
        Direction::Left => GameDirection::Left,
        Direction::Right => GameDirection::Right,
        Direction::Up => GameDirection::Up,
        Direction::Down => GameDirection::Down,
    }
}

impl<R: Renderer, E: EventSource> Tui48<R, E> {
    fn resize(&mut self) -> Result<()> {
        let (width, height) = self.renderer.size_hint()?;
        self.canvas = Canvas::new(width as usize, height as usize);
        self.tui_board = match Tui48Board::new(&self.board, &mut self.canvas) {
            Ok(tb) => Some(tb),
            Err(Error::TerminalTooSmall(_, _)) => None,
            Err(e) => return Err(e),
        };
        Ok(())
    }

    fn shift(&mut self, direction: Direction) -> Result<()> {
        let direction = direction_to_game_direction(direction);
        if let Some(_hint) = self.board.shift(direction) {
            let tb = self.tui_board.take();
            drop(tb);
            self.tui_board = Some(Tui48Board::new(&self.board, &mut self.canvas)?);
        }
        Ok(())
    }
}
