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
struct Tui48Board {
    canvas: Canvas,
    _board: DrawBuffer,
    _score: DrawBuffer,
    slots: Vec<Vec<Slot>>,
}

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
            Idx(BOARD_FIXED_X_OFFSET, BOARD_FIXED_Y_OFFSET, BOARD_LAYER_IDX),
            Bounds2D(36, 25),
        );
        let (cwidth, cheight) = canvas.dimensions();
        let (x_extent, y_extent) = board_rectangle.extents();
        if cwidth < x_extent || cheight < y_extent {
            return Err(TuiError::TerminalTooSmall(cwidth, cheight).into());
        }

        let mut board = canvas.get_draw_buffer(board_rectangle)?;
        board.draw_border()?;

        let mut score =
            canvas.get_draw_buffer(Rectangle(Idx(18, 1, BOARD_LAYER_IDX), Bounds2D(10, 3)))?;
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
                let mut opt = Slot::Empty;
                let value = round.get(&BoardIdx(x, y));
                if value > 0 {
                    let r = Self::tile_rectangle(x, y, TILE_LAYER_IDX);
                    let mut card_buffer = canvas.get_draw_buffer(r)?;
                    Tui48Board::draw_tile(&mut card_buffer, value)?;
                    opt = Slot::Static(Tile::new(BoardIdx(x, y), card_buffer));
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
            canvas: canvas.clone(),
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

    fn draw_tile(dbuf: &mut DrawBuffer, value: u16) -> Result<()> {
        let colors = colors_from_value(value);
        dbuf.modify(colors.0);
        dbuf.modify(colors.1);
        dbuf.draw_border()?;
        dbuf.fill(' ')?;
        dbuf.write_center(&format!("{}", value))?;
        Ok(())
    }

    fn get_slot(&mut self, idx: &BoardIdx) -> Result<Slot> {
        let s = self
            .slots
            .get_mut(idx.y())
            .ok_or(Error::UnableToRetrieveSlot {
                context: format!("getting row {}", idx.y()),
            })?
            .get_mut(idx.x())
            .ok_or(Error::UnableToRetrieveSlot {
                context: format!("getting slot at {},{}", idx.x(), idx.y()),
            })?;
        let s = s.take();
        Ok(s)
    }

    fn put_slot(&mut self, idx: &BoardIdx, slot: Slot) -> Result<()> {
        let s = self
            .slots
            .get_mut(idx.y())
            .ok_or(Error::UnableToRetrieveSlot {
                context: format!("getting row {}", idx.y()),
            })?
            .get_mut(idx.x())
            .ok_or(Error::UnableToRetrieveSlot {
                context: format!("getting slot at {},{}", idx.x(), idx.y()),
            })?;
        let _ = s.replace(slot);
        Ok(())
    }

    fn new_sliding_tile(
        &mut self,
        to_idx: &BoardIdx,
        value: u16,
        direction: &Direction,
    ) -> Result<SlidingTile> {
        let db_rectangle = match direction {
            Direction::Left => {
                let r = Tui48Board::tile_rectangle(4, to_idx.y(), LOWER_ANIMATION_LAYER_IDX);
                r
            }
            Direction::Right => {
                let mut r = Tui48Board::tile_rectangle(0, to_idx.y(), LOWER_ANIMATION_LAYER_IDX);
                r.0 .0 -= 6;
                r
            }
            Direction::Up => {
                let r = Tui48Board::tile_rectangle(to_idx.x(), 4, LOWER_ANIMATION_LAYER_IDX);
                r
            }
            Direction::Down => {
                let mut r = Tui48Board::tile_rectangle(to_idx.x(), 0, LOWER_ANIMATION_LAYER_IDX);
                r.0 .1 -= 6;
                r
            }
        };
        let mut buf = self.canvas.get_draw_buffer(db_rectangle)?;
        Tui48Board::draw_tile(&mut buf, value)?;
        let t = Tile {
            idx: to_idx.clone(),
            buf,
        };

        let rectangle = Tui48Board::tile_rectangle(to_idx.x(), to_idx.y(), LOWER_ANIMATION_LAYER_IDX);
        let st = SlidingTile::new(t, rectangle);

        Ok(st)
    }

    fn setup_animation(&mut self, hint: AnimationHint) -> Result<()> {
        for (idx, hint) in hint.hints() {
            let mut slot = self.get_slot(&idx)?;
            let new_slot = match hint {
                Hint::ToIdx(to_idx) => {
                    Slot::to_sliding(slot, to_idx, None)?
                }
                Hint::NewValueToIdx(value, to_idx) => {
                    Slot::to_sliding(slot, to_idx, Some(value))?
                }
                Hint::NewTile(value, slide_direction) => {
                    let t = self.new_sliding_tile(&idx, value, &slide_direction)?;
                    Slot::Sliding(t)
                }
            };
            self.put_slot(&idx, new_slot)?;
        }
        Ok(())
    }

    fn teardown_animation(&mut self) -> Result<()> {
        for idx in self
            .slots
            .iter_mut()
            .map(|i| i.iter())
            .flatten()
            .filter(|s| Slot::is_sliding(*s))
            .map(|s| s.idx())
            .collect::<Result<Vec<_>>>()?
        {
            let slot = self.get_slot(&idx)?;
            let static_slot = Slot::to_static(slot)?;
            self.put_slot(&idx, static_slot)?
        }

        Ok(())
    }

    fn animate(&mut self) -> Result<bool> {
        let should_continue = self
            .slots
            .iter_mut()
            .flatten()
            .filter(|slot| Slot::is_animating(*slot))
            .map(|slot| slot.animate())
            .collect::<Result<Vec<bool>>>()?
            .iter()
            .fold(false, |b, n| b | n);
        Ok(should_continue)
    }
}

impl From<&BoardIdx> for Idx {
    fn from(board_idx: &BoardIdx) -> Idx {
        Idx(board_idx.0, board_idx.1, 0)
    }
}

#[derive(Default)]
enum Slot {
    #[default]
    Empty,
    Static(Tile),
    Sliding(SlidingTile),
}

impl Slot {
    fn replace(&mut self, other: Self) -> Self {
        std::mem::replace(self, other)
    }

    fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    fn to_sliding(this: Self, to_idx: BoardIdx, new_value: Option<u16>) -> Result<Self> {
        // only allow static tiles to be converted to sliding
        let mut t = match this {
            Self::Static(t) => t,
            Self::Empty => {
                return Err(Error::CannotConvertToSliding {
                    idx: to_idx.clone(),
                })
            }
            Self::Sliding(_) => {
                return Err(Error::CannotConvertToSliding {
                    idx: to_idx.clone(),
                })
            }
        };

        t.buf.switch_layer(UPPER_ANIMATION_LAYER_IDX)?;
        if let Some(v) = new_value {
            Tui48Board::draw_tile(&mut t.buf, v)?;
        } else {
            Tui48Board::draw_tile(&mut t.buf, 5)?;
        }
        let to_rectangle = Tui48Board::tile_rectangle(to_idx.0, to_idx.1, UPPER_ANIMATION_LAYER_IDX);
        let st = SlidingTile::new(t, to_rectangle);

        Ok(Slot::Sliding(st))
    }

    fn to_static(this: Self) -> Result<Self> {
        // only allow static tiles to be converted to sliding
        if let Self::Static(_) = this {
            return Ok(this);
        }

        if let Self::Sliding(st) = this {
            let t = st.to_tile();
            t.buf.switch_layer(TILE_LAYER_IDX)?;
            return Ok(Slot::Static(t));
        }

        Err(Error::CannotConvertToStatic)
    }

    fn idx(&self) -> Result<BoardIdx> {
        //BoardIdx::default()
        match self {
            Slot::Empty => unreachable!(),
            Slot::Static(t) => Ok(t.idx.clone()),
            Slot::Sliding(st) => Ok(st.inner.idx.clone()),
        }
    }

    fn is_sliding(this: &Self) -> bool {
        match this {
            Slot::Sliding(_) => true,
            _ => false,
        }
    }

    fn is_animating(this: &Self) -> bool {
        match this {
            Slot::Empty => false,
            Slot::Static(_) => false,
            Slot::Sliding(st) => st.is_animating(),
        }
    }

    fn animate(&mut self) -> Result<bool> {
        match self {
            Slot::Empty => Ok(false),
            Slot::Static(_) => Ok(false),
            Slot::Sliding(st) => st.animate(),
        }
    }
}

struct Tile {
    idx: BoardIdx,
    buf: DrawBuffer,
}

impl Tile {
    fn new(idx: BoardIdx, buf: DrawBuffer) -> Self {
        Self { idx, buf }
    }
}

struct SlidingTile {
    inner: Tile,
    to_rectangle: Rectangle,
    is_animating: bool,
}

impl SlidingTile {
    fn new(inner: Tile, to_rectangle: Rectangle) -> Self {
        Self {
            inner,
            to_rectangle,
            is_animating: true,
        }
    }

    fn to_tile(self) -> Tile {
        self.inner
    }

    fn is_animating(&self) -> bool {
        self.is_animating
    }

    fn animate(&mut self) -> Result<bool> {
        if !self.is_animating {
            return Ok(false);
        }

        if self.inner.buf.rectangle().0.x() == self.to_rectangle.0.x()
            && self.inner.buf.rectangle().0.y() == self.to_rectangle.0.y()
        {
            // final frame
            self.inner.buf.switch_layer(TILE_LAYER_IDX)?;
            self.is_animating = false;
            return Ok(false);
        }
        let moving_idx = self.inner.buf.rectangle().0;
        let to_idx = &self.to_rectangle.0;
        let moving_buf = &self.inner.buf;
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
                let mut tui_board = self
                    .tui_board
                    .take()
                    .expect("why wouldn't we have a tui board at this point?");
                tui_board.setup_animation(hint)?;
                while tui_board.animate()? {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    self.renderer.render(&self.canvas)?;
                }
                tui_board.teardown_animation()?;
                self.tui_board = Some(tui_board);
            }
        }
        Ok(())
    }
}
