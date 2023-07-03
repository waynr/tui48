use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
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
const BOARD_FIXED_Y_OFFSET: usize = 5;
const BOARD_FIXED_X_OFFSET: usize = 5;
const BOARD_BORDER_WIDTH: usize = 1;
const BOARD_X_PADDING: usize = 2;
const BOARD_Y_PADDING: usize = 1;
const TILE_HEIGHT: usize = 5;
const TILE_WIDTH: usize = 6;

const BOARD_LAYER_IDX: usize = 2;
const LOWER_ANIMATION_LAYER_IDX: usize = 3;
const TILE_LAYER_IDX: usize = 4;
const UPPER_ANIMATION_LAYER_IDX: usize = 5;

impl Tui48Board {
    fn new(game: &Board, canvas: &mut Canvas) -> Result<Self> {
        let board_rectangle = Rectangle(
            Idx(BOARD_FIXED_X_OFFSET, BOARD_FIXED_Y_OFFSET, 0),
            Bounds2D(36, 25),
        );
        let (cwidth, cheight) = canvas.dimensions();
        let (x_extent, y_extent) = board_rectangle.extents();
        if cwidth < x_extent || cheight < y_extent {
            return Err(TuiError::TerminalTooSmall(cwidth, cheight).into());
        }

        let mut board = canvas.get_draw_buffer(board_rectangle)?;
        board.draw_border()?;

        let mut score = canvas.get_draw_buffer(Rectangle(Idx(18, 1, 0), Bounds2D(10, 3)))?;
        score.draw_border()?;
        score.fill(' ')?;
        score.write_right(&format!("{}", game.score()))?;
        score.modify(Modifier::SetBackgroundColor(75, 50, 25));
        score.modify(Modifier::SetForegroundColor(0, 0, 0));
        score.modify(Modifier::SetFGLightness(0.2));
        score.modify(Modifier::SetBGLightness(0.8));

        let (width, height) = game.dimensions();
        let round = game.current();
        let mut slots = Vec::with_capacity(height);
        let x_offset = BOARD_FIXED_X_OFFSET + BOARD_BORDER_WIDTH + BOARD_X_PADDING;
        let y_offset = BOARD_FIXED_Y_OFFSET + BOARD_BORDER_WIDTH;
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let mut opt = None;
                let value = round.get(&BoardIdx(x, y));
                if value > 0 {
                    let r = Self::tile_rectangle(x, y, 5);
                    let mut card_buffer = canvas.get_draw_buffer(r)?;
                    let colors = colors_from_value(value);
                    card_buffer.modify(colors.0);
                    card_buffer.modify(colors.1);
                    card_buffer.draw_border()?;
                    card_buffer.fill(' ')?;
                    card_buffer.write_center(&format!("{}", value))?;
                    opt = Some(card_buffer);
                }
                row.push(opt);
            }
            slots.push(row);
        }

        board.fill(' ')?;
        board.modify(Modifier::SetBackgroundColor(40, 0, 0));
        board.modify(Modifier::SetBGLightness(0.2));
        board.modify(Modifier::SetForegroundColor(25, 50, 75));
        board.modify(Modifier::SetFGLightness(0.6));
        Ok(Self {
            _board: board,
            _score: score,
            slots,
        })
    }

    fn tile_rectangle(x: usize, y: usize, z: usize) -> Rectangle {
        let x_offset = BOARD_FIXED_X_OFFSET + BOARD_BORDER_WIDTH + BOARD_X_PADDING;
        let y_offset = BOARD_FIXED_Y_OFFSET + BOARD_BORDER_WIDTH;
        let idx = Idx(
            x_offset + (BOARD_X_PADDING + TILE_WIDTH) * x,
            y_offset + (BOARD_Y_PADDING + TILE_HEIGHT) * y,
            z,
        );
        let bounds = Bounds2D(TILE_WIDTH, TILE_HEIGHT);
        Rectangle(idx, bounds)
    }
}

impl From<&BoardIdx> for Idx {
    fn from(board_idx: &BoardIdx) -> Idx {
        Idx(board_idx.0, board_idx.1, 0)
    }
}

struct AnimatedTui48Board {
    canvas: Canvas,
    new_tile: Option<Rc<RefCell<DrawBuffer>>>,
    displace_tile: Option<Rc<RefCell<DrawBuffer>>>,
    tui_board: Tui48Board,
    animation_hint: AnimationHint,
}

impl AnimatedTui48Board {
    fn new(canvas: Canvas, tui_board: Tui48Board, animation_hint: AnimationHint) -> Self {
        Self {
            canvas,
            new_tile: None,
            displace_tile: None,
            tui_board,
            animation_hint,
        }
    }

    fn animate(&mut self) -> Result<bool> {
        let hints = self.animation_hint.hints();
        Ok(hints
            .iter()
            .into_iter()
            .map(|(idx, hint)| {
                let idx: Idx = idx.into();
                match hint {
                    Hint::NewValueToIdx(new_value, to_idx) => {
                        let to_idx: Idx = to_idx.into();
                        let shifting_continue = self.animate_shifting_tile(
                            Some(*new_value),
                            idx.clone(),
                            to_idx.clone(),
                        )?;
                        self.animate_displaced_tile(to_idx.clone(), shifting_continue)?;
                        Ok(shifting_continue)
                    }
                    Hint::ToIdx(to_idx) => {
                        let to_idx: Idx = to_idx.into();
                        let shifting_continue =
                            self.animate_shifting_tile(None, idx.clone(), to_idx.clone())?;
                        self.animate_displaced_tile(to_idx.clone(), shifting_continue)?;
                        Ok(shifting_continue)
                    }
                    Hint::NewFrom(new_value, from_dir) => {
                        self.animate_new_tile(*new_value, idx.clone(), from_dir.clone())
                    }
                }
            })
            .collect::<Result<Vec<_>>>()?
            .iter()
            .all(|v| *v))
    }

    fn animate_displaced_tile(&mut self, displace_idx: Idx, last_frame: bool) -> Result<bool> {
        let displace_buf = match self.tui_board.slots[displace_idx.y()][displace_idx.x()].take() {
            Some(db) => db,
            None => return Ok(false),
        };
        let displace_tile = match &self.displace_tile {
            None => {
                // copy buffer to bottom layer as self.displace_tile
                let dbuf = Rc::new(RefCell::new(
                    displace_buf.clone_to(LOWER_ANIMATION_LAYER_IDX)?,
                ));
                // drop old buffer, should trigger clear of buffer and return of tuxels to the
                // canvas
                drop(displace_buf);
                self.displace_tile = Some(dbuf.clone());
                dbuf
            }
            Some(dbuf) => dbuf.clone(),
        };

        if last_frame {
            // on the last frame, drop the buffer
            self.displace_tile = None;
            return Ok(false);
        }

        let mut displace_tile = displace_tile.borrow_mut();
        displace_tile.modify(Modifier::AdjustLightnessBG(-0.1));
        displace_tile.modify(Modifier::AdjustLightnessFG(-0.1));

        Ok(true)
    }

    fn animate_shifting_tile(
        &mut self,
        new_value: Option<u16>,
        moving_idx: Idx,
        to_idx: Idx,
    ) -> Result<bool> {
        let target_rectangle = Tui48Board::tile_rectangle(to_idx.x(), to_idx.y(), TILE_LAYER_IDX);

        let moving_rectangle = self.tui_board.slots[moving_idx.y()][moving_idx.x()]
            .as_ref()
            .ok_or(Error::UnableToRetrieveDrawBuffer {
                reason: String::from("meow3"),
            })?
            .rectangle();

        // we check for animation termination before doing translation to ensure at least one frame
        // with no translation is available
        if moving_rectangle.x() == target_rectangle.x()
            && moving_rectangle.y() == target_rectangle.y()
        {
            // take ownership of card from its previous slot
            let mut moving_buf = self.tui_board.slots[moving_idx.y()][moving_idx.x()]
                .take()
                .expect("expect the buffer we've been working with not to be empty");

            // on last frame: update content if there is a new value
            if let Some(new_value) = new_value {
                let colors = colors_from_value(new_value);
                moving_buf.modify(colors.0);
                moving_buf.modify(colors.1);
                moving_buf.draw_border()?;
                moving_buf.fill(' ')?;
                moving_buf.write_center(&format!("{}", new_value))?;
            }

            // move buffer into destination slot on the tui_board
            let _ = self.tui_board.slots[to_idx.y()][to_idx.x()].replace(moving_buf);

            return Ok(false);
        }

        let moving_buf = self.tui_board.slots[moving_idx.y()][moving_idx.x()]
            .as_mut()
            .ok_or(Error::UnableToRetrieveDrawBuffer {
                reason: String::from("meow4"),
            })?;

        // 1 frame of buffer translation
        match (
            moving_idx.x() as i16 - to_idx.x() as i16,
            moving_idx.y() as i16 - to_idx.y() as i16,
        ) {
            (0, 0) => Ok(true), //no translation necessary
            (x, y) if x != 0 && y != 0 && x.abs() > y.abs() && x > 0 => {
                moving_buf.translate(Direction::Left)?;
                Ok(true)
            }
            (x, y) if x != 0 && y != 0 && x.abs() > y.abs() && x < 0 => {
                moving_buf.translate(Direction::Right)?;
                Ok(true)
            }
            (x, y) if x != 0 && y != 0 && x.abs() < y.abs() && y > 0 => {
                moving_buf.translate(Direction::Up)?;
                Ok(true)
            }
            (x, y) if x != 0 && y != 0 && x.abs() < y.abs() && y < 0 => {
                moving_buf.translate(Direction::Down)?;
                Ok(true)
            }
            (x, y) if x != 0 && y != 0 && x.abs() == y.abs() && y > 0 => {
                moving_buf.translate(Direction::Up)?;
                Ok(true)
            }
            (x, y) if x != 0 && y != 0 && x.abs() == y.abs() && y < 0 => {
                moving_buf.translate(Direction::Down)?;
                Ok(true)
            }
            (x, 0) if x > 0 => {
                moving_buf.translate(Direction::Left)?;
                Ok(true)
            }
            (x, 0) if x < 0 => {
                moving_buf.translate(Direction::Right)?;
                Ok(true)
            }
            (0, y) if y > 0 => {
                moving_buf.translate(Direction::Up)?;
                Ok(true)
            }
            (0, y) if y < 0 => {
                moving_buf.translate(Direction::Down)?;
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    fn animate_new_tile(
        &mut self,
        new_value: u16,
        to_idx: Idx,
        from_dir: Direction,
    ) -> Result<bool> {
        let new_tile = match &self.new_tile {
            None => {
                // generate new tile
                let from_idx = match from_dir {
                    Direction::Left => Idx(0, to_idx.y(), 0),
                    Direction::Right => Idx(3, to_idx.y(), 0),
                    Direction::Up => Idx(to_idx.x(), 3, 0),
                    Direction::Down => Idx(to_idx.x(), 0, 0),
                };
                let origin_rectangle =
                    Tui48Board::tile_rectangle(from_idx.x(), from_idx.y(), TILE_LAYER_IDX);

                let dbuf = Rc::new(RefCell::new(self.canvas.get_draw_buffer(origin_rectangle)?));
                self.new_tile = Some(dbuf.clone());
                dbuf.borrow_mut().write_center(&format!("{}", new_value))?;
                dbuf
            }
            Some(dbuf) => dbuf.clone(),
        };

        // compare new tile rectangle with target to determine if it's time to terminate. on
        // termination, assign new tile to the Tui48Board slot where it belongs
        {
            if new_tile.borrow().rectangle().0 == to_idx {
                self.new_tile = None;
                let t = Rc::into_inner(new_tile)
                    .expect("there should only be one strong reference to new_tile at this point")
                    .into_inner();
                self.tui_board.slots[to_idx.x()][to_idx.y()] = Some(t);
                return Ok(false);
            }
        }

        // 1 frame of buffer translation
        {
            new_tile.borrow_mut().translate(shift_dir)?;
        }

        Ok(true)
    }

    fn extract_board(self) -> Tui48Board {
        self.tui_board
    }
}

struct Colors {
    // TODO: change this from canvas::Modifer to colors::Rgb
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
                        Lch::new(80.0, 90.0, i as f32 * 360.0 / 10.0),
                        Lch::new(20.0, 50.0, fg_hue),
                    )
                })
                .map(|(k, bg_hsv, fg_hsv)| {
                    (
                        k,
                        Srgb::from_color(bg_hsv).into_format::<u8>(),
                        Srgb::from_color(fg_hsv).into_format::<u8>(),
                    )
                })
                .map(|(k, bg_rgb, fg_rgb)| {
                    (
                        k,
                        (
                            Modifier::SetBackgroundColor(bg_rgb.red, bg_rgb.green, bg_rgb.blue),
                            Modifier::SetForegroundColor(fg_rgb.red, fg_rgb.green, fg_rgb.blue),
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
            Modifier::SetBackgroundColor(255, 255, 255),
            Modifier::SetForegroundColor(90, 0, 0),
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
            redraw_entire: false,
            renderer,
            event_source,
            canvas: Canvas::new(width as usize, height as usize),
            tui_board: None,
        })
    }

    pub(crate) fn run(mut self) -> Result<()> {
        match self.inner_run() {
            Err(e) => {
                self.renderer.recover();
                Err(e)
            }
            Ok(_) => Ok(()),
        }
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    pub(crate) fn inner_run(&mut self) -> Result<()> {
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
                );
                while animation.animate()? {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    self.renderer.render(&self.canvas)?;
                }
                self.tui_board = Some(animation.extract_board());
            }
        }
        Ok(())
    }
}
