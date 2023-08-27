use std::collections::HashMap;
use std::sync::OnceLock;

use palette::{FromColor, Lch, Srgb};
use rand::thread_rng;

use crate::engine::board::Board;
use crate::engine::round::Idx as BoardIdx;
use crate::engine::round::{AnimationHint, Hint};

use super::error::{Error, Result};
use crate::tui::canvas::{Canvas, Modifier};
use crate::tui::textbuffer::{FormatOptions, HAlignment, VAlignment, TextBuffer};
use crate::tui::drawbuffer::{DrawBuffer, DrawBufferOwner};
use crate::tui::error::InnerError as TuiError;
use crate::tui::events::{Event, EventSource, UserInput};
use crate::tui::geometry::{Bounds2D, Direction, Idx, Rectangle};
use crate::tui::renderer::Renderer;

/// TUI representation of a 2048 game board.
struct Tui48Board {
    canvas: Canvas,
    board: DrawBuffer,
    score: TextBuffer,
    slots: Vec<Vec<Slot>>,
    disappearing_slots: Vec<Slot>,
    moving_slots: Vec<Slot>,
    done_slots: HashMap<BoardIdx, Slot>,
}

const BOARD_FIXED_Y_OFFSET: usize = 5;
const BOARD_FIXED_X_OFFSET: usize = 5;
const BOARD_BORDER_WIDTH: usize = 1;
const BOARD_X_PADDING: usize = 1;
const BOARD_Y_PADDING: usize = 1;
const TILE_HEIGHT: usize = 5;
const TILE_WIDTH: usize = 6;
const NEW_TILE_HORIZONTAL_OFFSET: usize = 4;
const NEW_TILE_VERTICAL_OFFSET: usize = 4;

const BOARD_LAYER_IDX: usize = 2;
const LOWER_ANIMATION_LAYER_IDX: usize = 3;
const TILE_LAYER_IDX: usize = 4;
const UPPER_ANIMATION_LAYER_IDX: usize = 5;

impl Tui48Board {
    fn new(game: &Board, canvas: &mut Canvas) -> Result<Self> {
        let (board_rectangle, score_rectangle) = Self::get_dimensions();

        let mut board = canvas.get_draw_buffer(board_rectangle)?;
        board.draw_border()?;

        let mut score = canvas.get_text_buffer(score_rectangle)?;
        Self::draw_score(&mut score, game.score())?;

        let (width, height) = game.dimensions();
        let round = game.current();
        let mut slots = Vec::with_capacity(height);
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let mut opt = Slot::Empty;
                let value = round.get(&BoardIdx(x, y));
                if value > 0 {
                    let r = Self::tile_rectangle(x, y, TILE_LAYER_IDX);
                    let mut card_buffer = canvas.get_text_buffer(r)?;
                    Tui48Board::draw_tile(&mut card_buffer, value)?;
                    opt = Slot::Static(Tile::new(value, BoardIdx(x, y), card_buffer));
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
            board: board,
            score,
            slots,
            moving_slots: Vec::new(),
            done_slots: HashMap::new(),
            disappearing_slots: Vec::new(),
        })
    }

    fn get_dimensions() -> (Rectangle, Rectangle) {
        let board_rectangle = Self::board_rectangle();
        let score_rectangle = Rectangle(Idx(18, 1, BOARD_LAYER_IDX), Bounds2D(10, 3));

        (board_rectangle, score_rectangle)
    }

    fn check_bounds(&self) -> Result<()> {
        let board_rectangle_with_tile_start = self
            .board
            .rectangle()
            .expand_by(NEW_TILE_HORIZONTAL_OFFSET, NEW_TILE_VERTICAL_OFFSET);

        let combined_rectangle = &board_rectangle_with_tile_start + &self.score.rectangle();
        let (x_extent, y_extent) = combined_rectangle.extents();

        let (cwidth, cheight) = self.canvas.dimensions();
        if cwidth < x_extent || cheight < y_extent {
            return Err(Error::TerminalTooSmall(cwidth, cheight).into());
        }

        Ok(())
    }

    #[cfg(test)]
    fn get_minimum_canvas_extents() -> (usize, usize) {
        let (board_rectangle, score_rectangle) = Self::get_dimensions();
        let board_rectangle_with_tile_start =
            board_rectangle.expand_by(NEW_TILE_HORIZONTAL_OFFSET, NEW_TILE_VERTICAL_OFFSET);

        let combined_rectangle = &board_rectangle_with_tile_start + &score_rectangle;

        combined_rectangle.extents()
    }

    fn board_rectangle() -> Rectangle {
        let x_bound: usize = TILE_WIDTH * 4 + BOARD_FIXED_X_OFFSET + BOARD_BORDER_WIDTH * 2;
        let y_bound: usize = TILE_HEIGHT * 4 + BOARD_FIXED_Y_OFFSET;

        Rectangle(
            Idx(BOARD_FIXED_X_OFFSET, BOARD_FIXED_Y_OFFSET, BOARD_LAYER_IDX),
            Bounds2D(x_bound, y_bound),
        )
    }

    fn tile_rectangle(x: usize, y: usize, z: usize) -> Rectangle {
        let x_offset = BOARD_FIXED_X_OFFSET + BOARD_BORDER_WIDTH * 2;
        let y_offset = BOARD_FIXED_Y_OFFSET + BOARD_BORDER_WIDTH;
        let idx = Idx(
            x_offset + (BOARD_X_PADDING + TILE_WIDTH) * x,
            y_offset + (BOARD_Y_PADDING + TILE_HEIGHT) * y,
            z,
        );
        let bounds = Bounds2D(TILE_WIDTH, TILE_HEIGHT);
        Rectangle(idx, bounds)
    }

    fn draw_tile(dbuf: &mut TextBuffer, value: u8) -> Result<()> {
        let colors = colors_from_value(value);
        dbuf.modify(colors.0);
        dbuf.modify(colors.1);
        dbuf.draw_border()?;
        dbuf.clear()?;
        dbuf.format(FormatOptions{
            halign: HAlignment::Center,
            valign: VAlignment::Middle,
        });
        dbuf.write(&format!("{}", 2u32.pow(value as u32)), None, None);
        dbuf.flush()?;
        Ok(())
    }

    fn draw_score(dbuf: &mut TextBuffer, value: u32) -> Result<()> {
        dbuf.draw_border()?;
        dbuf.clear()?;
        dbuf.write(&format!("{}", value), None, None);
        dbuf.flush()?;
        dbuf.modify(Modifier::SetBackgroundColor(75, 50, 25));
        dbuf.modify(Modifier::SetForegroundColor(0, 0, 0));
        dbuf.modify(Modifier::SetFGLightness(0.2));
        dbuf.modify(Modifier::SetBGLightness(0.8));
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
        value: u8,
        direction: &Direction,
    ) -> Result<SlidingTile> {
        let db_rectangle = match direction {
            Direction::Left => {
                let mut r = Tui48Board::tile_rectangle(3, to_idx.y(), LOWER_ANIMATION_LAYER_IDX);
                r.0 .0 += NEW_TILE_HORIZONTAL_OFFSET;
                r
            }
            Direction::Right => {
                let mut r = Tui48Board::tile_rectangle(0, to_idx.y(), LOWER_ANIMATION_LAYER_IDX);
                r.0 .0 -= NEW_TILE_HORIZONTAL_OFFSET;
                r
            }
            Direction::Up => {
                let mut r = Tui48Board::tile_rectangle(to_idx.x(), 3, LOWER_ANIMATION_LAYER_IDX);
                r.0 .1 += NEW_TILE_VERTICAL_OFFSET;
                r
            }
            Direction::Down => {
                let mut r = Tui48Board::tile_rectangle(to_idx.x(), 0, LOWER_ANIMATION_LAYER_IDX);
                r.0 .1 -= NEW_TILE_VERTICAL_OFFSET;
                r
            }
        };
        log::trace!("getting new textbuffer for rectangle {}", db_rectangle);
        let buf = self.canvas.get_text_buffer(db_rectangle)?;
        let mut t = Tile::new(value, to_idx.clone(), buf);
        t.draw()?;

        let rectangle =
            Tui48Board::tile_rectangle(to_idx.x(), to_idx.y(), LOWER_ANIMATION_LAYER_IDX);
        let st = SlidingTile::new(t, rectangle, None);

        Ok(st)
    }

    fn setup_animation(&mut self, hints: &AnimationHint) -> Result<()> {
        log::trace!("setting up animation with hints:\n{0}", hints);
        for (idx, hint) in hints.hints() {
            log::trace!("setting up animation for hint {0} -> {1}", idx, hint);
            let slot = self.get_slot(&idx)?;
            let new_slot = match hint.clone() {
                Hint::ToIdx(to_idx) => Slot::to_sliding(slot, to_idx, None)?,
                Hint::NewValueToIdx(value, to_idx) => Slot::to_sliding(slot, to_idx, Some(value))?,
                Hint::NewTile(value, slide_direction) => {
                    let t = self.new_sliding_tile(&idx, value, &slide_direction)?;
                    Slot::Sliding(t)
                }
            };
            self.moving_slots.push(new_slot);
            log::trace!(
                "Tui48Board after setting up hint {0} -> {1}:\n{2}",
                idx,
                hint,
                self
            );
            log::trace!(
                "Canvas after setting up animation for hint\n{}",
                self.canvas
            );
        }
        Ok(())
    }

    fn teardown_animation(&mut self) -> Result<()> {
        log::trace!("tearing down animation");
        log::trace!("current canvas:\n{}", self.canvas);
        for slot in self
            .done_slots
            .drain()
            .map(|(_, slot)| Slot::to_static(slot))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
        {
            let idx = slot.idx()?;
            self.put_slot(&idx, slot)?;
        }

        let _ = self.moving_slots.drain(0..);

        Ok(())
    }

    fn animate(&mut self) -> Result<bool> {
        log::trace!("about to animate a frame");
        let should_continue = self
            .moving_slots
            .iter_mut()
            .chain(self.disappearing_slots.iter_mut())
            .map(|slot| {
                match slot {
                    Slot::Empty => return Ok(false),
                    _ => (),
                }
                let idx = slot.idx()?;
                if let Some(bidx) = slot.board_index() {
                    log::trace!("about to animate slot {}\n{}", bidx, slot);
                }
                let c = slot.animate()?;
                if !c {
                    let new_done_slot = match self.done_slots.get_mut(&idx) {
                        // if there is a matching done slot for the current slot's index, then we
                        // need to decide which to keep and avoid tearing down the animation twice
                        // on the same index
                        Some(done_slot) => Self::keep_largest_value_tile(done_slot, slot),

                        // the slot we've been working with is the new slot for this index
                        None => slot.take(),
                    };
                    match self.done_slots.insert(idx, new_done_slot) {
                        Some(s) => drop(s),
                        _ => (),
                    };
                }
                Ok(c)
            })
            .collect::<Result<Vec<bool>>>()?
            .iter()
            .fold(false, |b, n| b | n);
        log::trace!("finished animating a frame");
        Ok(should_continue)
    }

    // take ownership of the contents of the slot with the highest value tile, return a new slot
    // with the kept tile
    fn keep_largest_value_tile(slot1: &mut Slot, slot2: &mut Slot) -> Slot {
        match (slot1.new_value(), slot2.new_value()) {
            (Some(_), None) => {
                let s1 = slot1.take();
                let _ = slot2.take();
                s1
            }
            (None, Some(_)) => {
                let s2 = slot2.take();
                let _ = slot1.take();
                s2
            }
            // i don't think this branch is very likely or even possible, but just in case it is I
            // am adding a warning statement for the logs since this safe-ish approach to handling
            // it might otherwise let it go unnoticed
            (Some(v1), Some(v2)) => {
                log::warn!("");
                if v1 >= v2 {
                    return slot1.take();
                }
                slot2.take()
            }
            // how likely is it that both slots are trying to take up the same board index but
            // neither has a new value to give it preference? unlikely enough in my mind that
            // unreachable!() is safe here and we don't need to check the current value of the
            // tiles
            (None, None) => unreachable!(),
        }
    }
}

impl std::fmt::Display for Tui48Board {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (y, row) in self.slots.iter().enumerate() {
            write!(f, "{:=^23}", "")?;
            write!(f, "{:=^23}", "")?;
            write!(f, "{:=^23}", "")?;
            write!(f, "{:=^23}", "")?;
            write!(f, "\n")?;

            let lines: Vec<[String; 5]> = row
                .iter()
                .enumerate()
                .map(|(x, slot)| {
                    let slot = match slot {
                        Slot::Empty => self
                            .moving_slots
                            .iter()
                            .find(|s| match s {
                                Slot::Sliding(st) => st.inner.idx.x() == x && st.inner.idx.y() == y,
                                _ => false,
                            })
                            .or_else(|| {
                                self.done_slots
                                    .iter()
                                    .find(|(_, s)| match s {
                                        Slot::Sliding(st) => {
                                            st.inner.idx.x() == x && st.inner.idx.y() == y
                                        }
                                        _ => false,
                                    })
                                    .map(|(_, s)| s)
                            }),
                        _ => Some(slot),
                    };
                    if let Some(s) = slot {
                        [
                            if let Some(v) = s.value() {
                                format!("{}", v)
                            } else {
                                String::new()
                            },
                            if let Some(bidx) = s.board_index() {
                                format!("{}", bidx)
                            } else {
                                String::new()
                            },
                            if let Some(r) = s.rectangle() {
                                format!("{}", r.0)
                            } else {
                                String::new()
                            },
                            if let Some(v) = s.new_value() {
                                format!("{}", v)
                            } else {
                                String::new()
                            },
                            if let Some(r) = s.to_rectangle() {
                                format!("{}", r.0)
                            } else {
                                String::new()
                            },
                        ]
                    } else {
                        [
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                        ]
                    }
                })
                .collect();
            let vals = lines.iter().map(|s| &s[0]);
            let bidxs = lines.iter().map(|s| &s[1]);
            let cidxs = lines.iter().map(|s| &s[2]);
            let new_vals = lines.iter().map(|s| &s[3]);
            let to_cidxs = lines.iter().map(|s| &s[4]);

            for s in vals {
                write!(f, "{: <7}:", "val")?;
                write!(f, "{: >14}|", s)?;
            }
            write!(f, "\n")?;

            for s in bidxs {
                write!(f, "{: <7}:", "bidx")?;
                write!(f, "{: >14}|", s)?;
            }
            write!(f, "\n")?;

            for s in cidxs {
                write!(f, "{: <7}:", "cidx")?;
                write!(f, "{: >14}|", s)?;
            }
            write!(f, "\n")?;

            for s in new_vals {
                write!(f, "{: <7}:", "newval")?;
                write!(f, "{: >14}|", s)?;
            }
            write!(f, "\n")?;

            for s in to_cidxs {
                write!(f, "{: <7}:", "to_cidx")?;
                write!(f, "{: >14}|", s)?;
            }
            write!(f, "\n")?;

            write!(f, "{:.^23}", "")?;
            write!(f, "{:.^23}", "")?;
            write!(f, "{:.^23}", "")?;
            write!(f, "{:.^23}", "")?;
            write!(f, "\n")?;
        }
        Ok(())
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

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Empty => f.pad("empty")?,
            Self::Static(t) => write!(f, "{}", t)?,
            Self::Sliding(st) => write!(f, "{}", st)?,
        };
        Ok(())
    }
}

impl Slot {
    fn replace(&mut self, other: Self) -> Self {
        std::mem::replace(self, other)
    }

    fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    fn to_sliding(this: Self, to_idx: BoardIdx, new_value: Option<u8>) -> Result<Self> {
        // only allow static tiles to be converted to sliding
        let mut t = match this {
            Self::Static(t) => t,
            Self::Empty => return Err(Error::CannotConvertToSliding { idx: None }),
            Self::Sliding(_) => {
                return Err(Error::CannotConvertToSliding {
                    idx: Some(this.idx()?),
                })
            }
        };

        log::trace!(
            "about move buffer to layer {0}\n{1}",
            UPPER_ANIMATION_LAYER_IDX,
            t.buf
        );
        t.buf.switch_layer(UPPER_ANIMATION_LAYER_IDX)?;
        t.idx = to_idx.clone();
        if let Some(v) = new_value {
            t.value = v;
        }
        let to_rectangle =
            Tui48Board::tile_rectangle(to_idx.0, to_idx.1, UPPER_ANIMATION_LAYER_IDX);
        let st = SlidingTile::new(t, to_rectangle, new_value);

        Ok(Slot::Sliding(st))
    }

    fn to_static(this: Self) -> Result<Self> {
        if let Self::Static(_) = this {
            return Ok(this);
        }

        // only allow sliding tiles to be converted to static
        if let Self::Sliding(st) = this {
            let mut t = st.to_tile();
            t.buf.switch_layer(TILE_LAYER_IDX)?;
            t.draw()?;
            return Ok(Slot::Static(t));
        }

        Err(Error::CannotConvertToStatic)
    }

    fn idx(&self) -> Result<BoardIdx> {
        match self {
            Slot::Empty => unreachable!(),
            Slot::Static(t) => Ok(t.idx.clone()),
            Slot::Sliding(st) => Ok(st.inner.idx.clone()),
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

impl Slot {
    fn value(&self) -> Option<u8> {
        match self {
            Self::Empty => None,
            Self::Static(t) => Some(t.value()),
            Self::Sliding(st) => Some(st.value()),
        }
    }

    fn new_value(&self) -> Option<u8> {
        match self {
            Self::Empty => None,
            Self::Static(_) => None,
            Self::Sliding(st) => st.new_value(),
        }
    }

    fn board_index(&self) -> Option<BoardIdx> {
        match self {
            Self::Empty => None,
            Self::Static(t) => Some(t.board_index()),
            Self::Sliding(st) => Some(st.board_index()),
        }
    }

    fn rectangle(&self) -> Option<Rectangle> {
        match self {
            Self::Empty => None,
            Self::Static(t) => Some(t.rectangle()),
            Self::Sliding(st) => Some(st.rectangle()),
        }
    }

    fn to_rectangle(&self) -> Option<Rectangle> {
        match self {
            Self::Empty => None,
            Self::Static(_) => None,
            Self::Sliding(st) => Some(st.to_rectangle()),
        }
    }
}

struct Tile {
    value: u8,
    idx: BoardIdx,
    buf: TextBuffer,
}

impl std::fmt::Display for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "T({},{},{})",
            self.value,
            self.idx,
            self.buf.rectangle().0
        )
    }
}

impl Tile {
    fn new(value: u8, idx: BoardIdx, buf: TextBuffer) -> Self {
        Self { value, idx, buf }
    }

    fn draw(&mut self) -> Result<()> {
        Tui48Board::draw_tile(&mut self.buf, self.value)
    }

    fn value(&self) -> u8 {
        self.value
    }

    fn board_index(&self) -> BoardIdx {
        self.idx.clone()
    }

    fn rectangle(&self) -> Rectangle {
        self.buf.rectangle()
    }
}

struct SlidingTile {
    inner: Tile,
    to_rectangle: Rectangle,
    is_animating: bool,
    new_value: Option<u8>,
}

impl std::fmt::Display for SlidingTile {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(v) = self.new_value {
            write!(f, "ST({}->({},{}))", self.inner, self.to_rectangle.0, v,)
        } else {
            write!(f, "ST({}->({},N/A))", self.inner, self.to_rectangle.0,)
        }
    }
}

impl SlidingTile {
    fn new(inner: Tile, to_rectangle: Rectangle, new_value: Option<u8>) -> Self {
        Self {
            inner,
            to_rectangle,
            is_animating: true,
            new_value,
        }
    }

    fn to_tile(self) -> Tile {
        self.inner
    }

    fn animate(&mut self) -> Result<bool> {
        if !self.is_animating {
            return Ok(false);
        }

        if self.inner.buf.rectangle().0.x() == self.to_rectangle.0.x()
            && self.inner.buf.rectangle().0.y() == self.to_rectangle.0.y()
        {
            // final frame
            // don't move the textbuffer to the tile layer, leave that for
            // Tui48Board.teardown_animation
            if let Some(v) = self.new_value {
                self.inner.value = v;
            }
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

impl SlidingTile {
    fn value(&self) -> u8 {
        self.inner.value
    }

    fn new_value(&self) -> Option<u8> {
        self.new_value
    }

    fn board_index(&self) -> BoardIdx {
        self.inner.idx.clone()
    }

    fn to_rectangle(&self) -> Rectangle {
        self.to_rectangle.clone()
    }

    fn rectangle(&self) -> Rectangle {
        self.inner.buf.rectangle()
    }
}

struct Colors {
    // TODO: change this from canvas::Modifer to colors::Rgb
    card_colors: HashMap<u8, (Modifier, Modifier)>,
}

static DEFAULT_COLORS: OnceLock<Colors> = OnceLock::new();
static MAX_TILE_EXPONENT: u8 = 17;

pub(crate) fn init() -> Result<()> {
    if let Some(_) = DEFAULT_COLORS.get() {
        // already set, no need to do anything else
        return Ok(());
    }
    let bg_hue = 28.0;
    let fg_hue = bg_hue + 180.0;
    let defaults = Colors {
        card_colors: HashMap::from_iter(
            (0..MAX_TILE_EXPONENT)
                .into_iter()
                .map(|i| {
                    (
                        i,
                        Lch::new(80.0, 90.0 - (40.0 * ((i % 2) as f32)), i as f32 * 360.0 / MAX_TILE_EXPONENT as f32),
                        Lch::new(20.0, 90.0 - (40.0 * (((i + 1) % 2) as f32)), fg_hue),
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
    let _ = DEFAULT_COLORS.set(defaults);

    Ok(())
}

#[inline(always)]
fn colors_from_value(value: u8) -> (Modifier, Modifier) {
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

    pub(crate) fn run(mut self) -> Result<()> {
        let mut state = GameState::Active;
        loop {
            state = match state {
                GameState::Quit => return Ok(()),
                GameState::Reset => self.reset()?,
                GameState::TerminalTooSmall => match self.run_terminal_too_small() {
                    Err(e) => {
                        self.renderer.recover();
                        return Err(e);
                    }
                    Ok(state) => state,
                },
                GameState::Active => match self.run_game_active() {
                    Err(e) => {
                        self.renderer.recover();
                        return Err(e);
                    }
                    Ok(state) => state,
                },
                GameState::Over => match self.run_game_over() {
                    Err(e) => {
                        self.renderer.recover();
                        return Err(e);
                    }
                    Ok(state) => state,
                },
            }
        }
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    fn run_game_active(&mut self) -> Result<GameState> {
        self.tui_board = match self.resize()? {
            Some(tb) => Some(tb),
            None => return Ok(GameState::TerminalTooSmall),
        };

        loop {
            self.renderer.render(&self.canvas)?;
            log::trace!("rendered, waiting for input");
            match self.event_source.next_event()? {
                Event::UserInput(UserInput::Direction(d)) => {
                    let game_over = self.shift(d)?;
                    if game_over {
                        return Ok(GameState::Over);
                    }
                }
                Event::UserInput(UserInput::NewGame) => return Ok(GameState::Reset),
                Event::UserInput(UserInput::Quit) => break,
                Event::Resize => {
                    self.tui_board = match self.resize()? {
                        Some(tb) => Some(tb),
                        None => return Ok(GameState::TerminalTooSmall),
                    };
                }
            }
        }
        Ok(GameState::Quit)
    }

    fn run_game_over(&mut self) -> Result<GameState> {
        self.resize()?;

        if let Some(tui_board) = &self.tui_board {
            let board_rectangle = tui_board.board.rectangle();
            let message_rectangle = board_rectangle.shrink_by(5, 8);
            let mut buf = self.canvas.get_text_buffer(message_rectangle)?;
            buf.clear()?;
            buf.write("game over! press 'q' to quit or 'n' to start new game", None, None);
            buf.flush()?;
            self.renderer.render(&self.canvas)?;
            match self.event_source.next_event()? {
                Event::UserInput(UserInput::Direction(d)) => {
                    let game_over = self.shift(d)?;
                    if game_over {
                        return Ok(GameState::Over);
                    }
                }
                Event::UserInput(UserInput::NewGame) => return Ok(GameState::Reset),
                Event::UserInput(UserInput::Quit) => return Ok(GameState::Quit),
                Event::Resize => {
                    self.tui_board = match self.resize()? {
                        Some(tb) => Some(tb),
                        None => return Ok(GameState::TerminalTooSmall),
                    };
                }
            }
        }

        Ok(GameState::Active)
    }

    fn run_terminal_too_small(&mut self) -> Result<GameState> {
        self.renderer.clear(&self.canvas)?;
        loop {
            let (c_width, c_height) = self.canvas.dimensions();
            let canvas_rectangle = Rectangle(Idx(0,0,0), Bounds2D(c_width, c_height));
            let message_rectangle = canvas_rectangle.shrink_by(2, 2);
            let mut buf = self.canvas.get_text_buffer(message_rectangle)?;
            buf.clear()?;
            buf.write("the terminal is too small, please make it bigger!", None, None);
            buf.flush()?;
            self.renderer.render(&self.canvas)?;
            match self.event_source.next_event()? {
                Event::Resize => {
                    self.tui_board = match self.resize()? {
                        Some(tb) => Some(tb),
                        None => continue,
                    };
                    break;
                }
                _ => continue,
            }
        }
        self.renderer.clear(&self.canvas)?;
        if self.board.is_game_over() {
            Ok(GameState::Over)
        } else {
            Ok(GameState::Active)
        }
    }

    fn reset(&mut self) -> Result<GameState> {
        let rng = thread_rng();
        self.board = Board::new(rng);
        self.tui_board = self.resize()?;
        Ok(GameState::Active)
    }

    fn resize(&mut self) -> Result<Option<Tui48Board>> {
        let (width, height) = self.renderer.size_hint()?;
        self.canvas = Canvas::new(width as usize, height as usize);

        match Tui48Board::new(&self.board, &mut self.canvas) {
            Ok(tb) => match tb.check_bounds() {
                Err(_) => Ok(None),
                Ok(_) => Ok(Some(tb)),
            },
            Err(Error::TerminalTooSmall(_, _)) => Ok(None),
            Err(e) => match &e {
                Error::TuiError { source: tui_error } => match tui_error.inner {
                    TuiError::OutOfBoundsX(_) => Ok(None),
                    TuiError::OutOfBoundsY(_) => Ok(None),
                    _ => Err(e),
                },
                _ => Err(e),
            },
        }
    }

    fn shift(&mut self, direction: Direction) -> Result<bool> {
        let mut game_over = false;
        if let Some(hint) = self.board.shift(direction) {
            game_over = hint.game_over();
            let mut tui_board = self
                .tui_board
                .take()
                .expect("why wouldn't we have a tui board at this point?");
            Tui48Board::draw_score(&mut tui_board.score, self.board.score())?;
            log::trace!("Tui48Board prior to setting up animation\n{}", tui_board);
            log::trace!("Canvas prior to setting up animation\n{}", self.canvas);
            tui_board.setup_animation(&hint)?;
            log::trace!("after setting up animation\n{}", tui_board);
            let mut fc = 0;
            while tui_board.animate()? {
                log::trace!("generated animation frame {0}\n{1}", fc, tui_board);
                std::thread::sleep(std::time::Duration::from_millis(5));
                self.renderer.render(&self.canvas)?;
                log::trace!("rendered frame {} after sleeping 1ms", fc);

                fc += 1;
            }
            tui_board.teardown_animation()?;
            self.renderer.render(&self.canvas)?;
            let _ = self.tui_board.replace(tui_board);
        }
        Ok(game_over)
    }
}

enum GameState {
    Active,
    Over,
    Reset,
    TerminalTooSmall,
    Quit,
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use env_logger;
    use log::Log;
    use rand::SeedableRng;
    use rstest::*;

    use super::*;
    use crate::engine::round::Round;

    fn generate_round_from(idxs: HashMap<BoardIdx, u8>) -> Round {
        let mut round = Round::default();
        for x in 0..3 {
            for y in 0..3 {
                let idx = BoardIdx(x, y);
                if let Some(v) = idxs.get(&idx) {
                    round.set_value(&idx, v.clone());
                }
            }
        }
        round
    }

    fn setup(
        width: usize,
        height: usize,
        idxs: HashMap<BoardIdx, u8>,
    ) -> Result<(Board, Canvas, Tui48Board)> {
        let mut canvas = Canvas::new(width, height);
        let rng = rand::rngs::SmallRng::seed_from_u64(10);
        let mut game_board = Board::new(rng);
        let round = generate_round_from(idxs);
        game_board.set_initial_round(round);

        let tui_board = Tui48Board::new(&game_board, &mut canvas)?;
        Ok((game_board, canvas, tui_board))
    }

    fn verify_occupied_layers(c: &Canvas, occupied: Vec<usize>, not_occupied: Vec<usize>) {
        for zdx in occupied.iter() {
            assert!(c.layer_occupied(*zdx), "layer {} should be occupied", zdx)
        }
        for zdx in not_occupied.iter() {
            assert!(
                !c.layer_occupied(*zdx),
                "layer {} should not be occupied",
                zdx
            )
        }
    }

    fn debug(args: core::fmt::Arguments) -> log::Record {
        log::Record::builder()
            .level(log::Level::Debug)
            .args(args)
            .build()
    }

    #[test]
    fn test_slide() -> Result<()> {
        init()?;

        let logger = env_logger::Logger::from_default_env();

        let idxs = HashMap::from([(BoardIdx(0, 0), 2), (BoardIdx(0, 1), 2)]);
        let (mut game_board, canvas, mut tui_board) = setup(100, 100, idxs)?;

        let hint = game_board
            .shift(Direction::Down)
            .expect("down should definitely result in hints");
        assert_eq!(hint.hints().len(), 3);

        let hints = hint.hints();
        let (idx1, hint1) = hints.get(0).expect("expecting three hints");
        let (idx2, hint2) = hints.get(1).expect("expecting three hints");
        let (idx3, hint3) = hints.get(2).expect("expecting three hints");

        assert_eq!(*idx1, BoardIdx(0, 1));
        assert!(matches!(hint1, Hint::ToIdx(BoardIdx(0, 3))));
        assert_eq!(*idx2, BoardIdx(0, 0));
        assert!(matches!(hint2, Hint::NewValueToIdx(3, BoardIdx(0, 3))));
        assert_eq!(*idx3, BoardIdx(2, 0));
        assert!(matches!(hint3, Hint::NewTile(1, Direction::Down)));

        verify_occupied_layers(&canvas, vec![2, 4], vec![0, 1, 3, 5, 6, 7]);
        tui_board.setup_animation(&hint)?;
        verify_occupied_layers(&canvas, vec![2, 3, 5], vec![0, 1, 4, 6, 7]);

        // TODO: verify board after setup
        assert_eq!(tui_board.moving_slots.len(), 3);
        assert_eq!(tui_board.done_slots.len(), 0);
        assert_eq!(tui_board.disappearing_slots.len(), 0);

        while tui_board.animate()? {
            // TODO: verify intermediate states after every animation frame
            verify_occupied_layers(&canvas, vec![2, 3, 5], vec![0, 1, 4, 6, 7]);
            logger.log(&debug(format_args!(
                "moving slot count: {}",
                tui_board.moving_slots.len()
            )));
            logger.log(&debug(format_args!(
                "active moving slot count: {}",
                tui_board
                    .moving_slots
                    .iter()
                    .map(|s| match s {
                        Slot::Empty => 0,
                        _ => 1,
                    })
                    .sum::<u16>()
            )));
            logger.log(&debug(format_args!(
                "non-empty done slot count: {}",
                tui_board
                    .done_slots
                    .iter()
                    .map(|(_, s)| match s {
                        Slot::Empty => 0,
                        _ => 1,
                    })
                    .sum::<u16>()
            )));
            logger.log(&debug(format_args!(
                "done_slot count  : {}",
                tui_board.done_slots.len()
            )));
        }
        tui_board.teardown_animation()?;
        assert_eq!(tui_board.moving_slots.len(), 0);
        assert_eq!(tui_board.done_slots.len(), 0);
        assert_eq!(tui_board.disappearing_slots.len(), 0);
        verify_occupied_layers(&canvas, vec![2, 4], vec![0, 1, 3, 5, 6, 7]);
        // TODO: verify canvas after teardown

        Ok(())
    }

    #[rstest]
    #[case::zero(0, 0)]
    #[case::small(10, 10)]
    #[case::height_too_small(100, 24)]
    #[case::width_too_small(35, 100)]
    fn check_bounds_error_if_terminal_is_too_small_for_board(
        #[case] width: usize,
        #[case] height: usize,
    ) -> Result<()> {
        init()?;

        let idxs = HashMap::from([(BoardIdx(0, 0), 2), (BoardIdx(0, 1), 2)]);
        let r = setup(width, height, idxs);
        assert!(r.is_err());
        Ok(())
    }

    #[rstest]
    fn check_bounds_width_animation_errors(
        // TODO: try submitting feature to rstest to so we can do something like
        // #[range(36usize..66)]
        #[values(
            36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57,
            58, 59, 60, 61, 62, 63, 64, 65, 66
        )]
        width: usize,
    ) -> Result<()> {
        init()?;
        let height = 100usize;

        let idxs = HashMap::from([(BoardIdx(0, 0), 2), (BoardIdx(0, 1), 2)]);
        let r = setup(width, height, idxs);
        assert!(r.is_ok());
        let (_board, _canvas, tui48_board) = r.unwrap();
        let r = tui48_board.check_bounds();
        assert!(r.is_err());
        Ok(())
    }

    #[rstest]
    fn check_bounds_height_animation_errors(
        #[values(30, 31, 32, 33, 34, 35, 36)] height: usize,
    ) -> Result<()> {
        init()?;
        let width = 100usize;

        let idxs = HashMap::from([(BoardIdx(0, 0), 2), (BoardIdx(0, 1), 2)]);
        let r = setup(width, height, idxs);
        assert!(r.is_ok());
        let (_board, _canvas, tui48_board) = r.unwrap();
        let r = tui48_board.check_bounds();
        assert!(r.is_err());
        Ok(())
    }

    #[rstest]
    #[case::top(Direction::Down)]
    #[case::bottom(Direction::Up)]
    #[case::left(Direction::Right)]
    #[case::right(Direction::Left)]
    fn check_bounds_animation(#[case] slide_dir: Direction) -> Result<()> {
        init()?;

        let idxs = HashMap::from([(BoardIdx(1, 1), 2), (BoardIdx(2, 2), 2)]);
        let (x_extent, y_extent) = Tui48Board::get_minimum_canvas_extents();
        let (mut game_board, _, mut tui_board) = setup(x_extent, y_extent, idxs)?;

        let hint = game_board
            .shift(slide_dir.clone())
            .expect(format!("{:?} slide should result in hints", slide_dir).as_str());

        let r = Tui48Board::draw_score(&mut tui_board.score, game_board.score());
        assert!(r.is_ok());
        let r = tui_board.setup_animation(&hint);
        assert!(r.is_ok());
        while tui_board.animate()? {}
        let r = tui_board.teardown_animation();
        assert!(r.is_ok());

        Ok(())
    }
}
