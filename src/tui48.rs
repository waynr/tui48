use std::collections::HashMap;
use std::sync::OnceLock;

use palette::{FromColor, Lch, Srgb};

use crate::engine::board::Board;
use crate::engine::round::Idx as BoardIdx;
use crate::engine::round::{AnimationHint, Hint, Round};

use super::error::{Error, Result};
use crate::tui::canvas::{Canvas, Modifier};
use crate::tui::drawbuffer::DrawBuffer;
use crate::tui::error::TuiError;
use crate::tui::events::{Event, EventSource, UserInput};
use crate::tui::geometry::{Bounds2D, Direction, Idx, Rectangle};
use crate::tui::renderer::Renderer;

struct Tui48Board {
    _board: DrawBuffer,
    _score: DrawBuffer,
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
        let board_rectangle = Rectangle(Idx(5, 5, 0), Bounds2D(36, 25));
        let (cwidth, cheight) = canvas.dimensions();
        let (x_extent, y_extent) = board_rectangle.extents();
        if cwidth < x_extent || cheight < y_extent {
            return Err(TuiError::TerminalTooSmall(cwidth, cheight).into());
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
                    card_buffer.modify(Modifier::Bold);
                    let colors = colors_from_value(value);
                    card_buffer.modify(colors.0);
                    card_buffer.modify(colors.1);
                    card_buffer.draw_border()?;
                    card_buffer.fill(' ')?;
                    card_buffer.modify(Modifier::Bold);
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
            slots,
        })
    }
}

struct AnimatedTui48Board {
    canvas: Canvas,
    new_tile: Option<DrawBuffer>,
    tui_board: Tui48Board,
    animation_hint: AnimationHint,
    round: Round,
}

impl AnimatedTui48Board {
    fn new(
        canvas: Canvas,
        tui_board: Tui48Board,
        animation_hint: AnimationHint,
        round: Round,
    ) -> Self {
        Self {
            canvas,
            new_tile: None,
            tui_board,
            animation_hint,
            round,
        }
    }

    fn animate(&mut self) -> Result<bool> {
        let hints = self.animation_hint.hints();
        for (idx, hint) in hints.iter().into_iter() {
            let moving_db = &self.tui_board.slots[idx.y()][idx.x()]
                .as_mut()
                .ok_or(Error::UnableToRetrieveDrawBuffer);
            let target_rectangle = match hint {
                Hint::None => continue,
                Hint::ToIdx(to_idx) => self.tui_board.slots[to_idx.y()][to_idx.x()]
                    .as_mut()
                    .ok_or(Error::UnableToRetrieveDrawBuffer)?
                    .rectangle(),
                Hint::NewFrom(from_dir) => {
                    self.new_tile = Some(
                        self.canvas
                            .get_draw_buffer(Rectangle(Idx(0, 0, 0), Bounds2D(10, 10)))?,
                    );
                    let to_idx = idx;
                    self.tui_board.slots[to_idx.y()][to_idx.x()]
                        .as_mut()
                        .ok_or(Error::UnableToRetrieveDrawBuffer)?
                        .rectangle()
                }
            };
        }
        Ok(false)
    }

    fn animate_existing_tile(&mut self, from_dir: Direction) -> Result<bool> {
        Ok(false)
    }

    fn animate_new_tile(&mut self, from_dir: Direction) -> Result<bool> {
        Ok(false)
    }

    fn extract_board(self) -> Tui48Board {
        self.tui_board
    }
}

struct Colors {
    card_colors: HashMap<u16, (Modifier, Modifier)>,
}

static DEFAULT_COLORS: OnceLock<Colors> = OnceLock::new();

pub(crate) fn init() -> Result<()> {
    let bg_hue = 28.0;
    let fg_hue = bg_hue + 180.0;
    let defaults = Colors {
        card_colors: HashMap::from_iter(
            (0..11)
                .into_iter()
                .map(|i| {
                    (
                        2u16.pow(i),
                        Lch::new(80.0, 90.0, i as f32 * 360.0/10.0),
                        Lch::new(20.0, 50.0, fg_hue),
                    )
                })
                .map(|(k, bg_hsv, fg_hsv)| {
                    (
                        k,
                        (
                            Srgb::from_color(bg_hsv).into_format::<u8>(),
                            Srgb::from_color(fg_hsv).into_format::<u8>(),
                        ),
                    )
                })
                .map(|(k, (bg_rgb, fg_rgb))| {
                    (
                        k,
                        (
                            Modifier::BackgroundColor(bg_rgb.red, bg_rgb.green, bg_rgb.blue),
                            Modifier::ForegroundColor(fg_rgb.red, fg_rgb.green, fg_rgb.blue),
                        ),
                    )
                }),
        ),
    };
    DEFAULT_COLORS
        .set(defaults)
        .or(Err(Error::DefaultColorsAlreadySet))?;
    Ok(())
}

#[inline(always)]
fn colors_from_value(value: u16) -> (Modifier, Modifier) {
    let (background, foreground) = DEFAULT_COLORS
        .get()
        .expect("DEFAULT_COLORS should always be initialized by this point")
        .card_colors
        .get(&value)
        .unwrap_or(&(
            Modifier::BackgroundColor(255, 255, 255),
            Modifier::ForegroundColor(90, 0, 0),
        ));
    (background.clone(), foreground.clone())
}

pub(crate) struct Tui48<R: Renderer, E: EventSource> {
    renderer: R,
    redraw_entire: bool,
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
            redraw_entire: true,
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
                        Some(b) => {
                            drop(b);
                        }
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

impl<R: Renderer, E: EventSource> Tui48<R, E> {
    fn resize(&mut self) -> Result<()> {
        let (width, height) = self.renderer.size_hint()?;
        self.canvas = Canvas::new(width as usize, height as usize);
        self.tui_board = match Tui48Board::new(&self.board, &mut self.canvas) {
            Ok(tb) => Some(tb),
            Err(Error::TuiError {
                source: TuiError::TerminalTooSmall(_, _),
            }) => None,
            Err(e) => return Err(e),
        };
        Ok(())
    }

    fn shift(&mut self, direction: Direction) -> Result<()> {
        if let Some(hint) = self.board.shift(direction) {
            if self.redraw_entire {
                let tb = self.tui_board.take();
                drop(tb);
                self.tui_board = Some(Tui48Board::new(&self.board, &mut self.canvas)?);
            } else {
                let mut animation = AnimatedTui48Board::new(
                    self.canvas.clone(),
                    self.tui_board
                        .take()
                        .expect("tui_board should always be Some at this point"),
                    hint,
                    self.board.current(),
                );
                while animation.animate()? {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                self.tui_board = Some(animation.extract_board());
            }
        }
        Ok(())
    }
}
