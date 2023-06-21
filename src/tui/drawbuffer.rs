use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use super::canvas::{Modifier, SharedModifiers};
use super::error::Result;
use super::geometry::{Idx, Position, Rectangle};
use super::tuxel::Tuxel;

#[derive(Default)]
pub(crate) struct DrawBufferInner {
    pub(crate) rectangle: Rectangle,
    pub(crate) border: bool,
    pub(crate) buf: Vec<Vec<Tuxel>>,
    pub(crate) modifiers: SharedModifiers,
}

impl Drop for DrawBufferInner {
    fn drop(&mut self) {
        for row in self.buf.iter_mut() {
            for tuxel in row.iter_mut() {
                tuxel.clear();
            }
        }
    }
}

impl DrawBufferInner {
    fn write_left(&mut self, s: &str) -> Result<()> {
        let y = self.rectangle.height() / 2;
        let x = if self.border { 1 } else { 0 };
        for (offset, c) in s.chars().enumerate() {
            if offset > self.rectangle.width() + x {
                // can't write more than width of buffer
                break;
            }
            self.get_tuxel(Position::Idx(offset + x, y)).set_content(c);
        }
        Ok(())
    }

    fn write_right(&mut self, s: &str) -> Result<()> {
        let y = self.rectangle.height() / 2;
        let x = if self.border {
            self.rectangle.width() - 2
        } else {
            self.rectangle.width() - 1
        };
        for (offset, c) in s.chars().rev().enumerate() {
            if offset > x + 1 {
                // can't write more than width of buffer
                break;
            }
            self.get_tuxel(Position::Idx(x - offset, y)).set_content(c);
        }
        Ok(())
    }

    fn write_center(&mut self, s: &str) -> Result<()> {
        let y_offset = self.rectangle.height() / 2;
        let width = self.rectangle.width();
        let available_width = if self.border { width - 2 } else { width };
        let border_offset = if self.border { 1 } else { 0 };
        let x_offset = if s.len() >= available_width {
            border_offset
        } else {
            border_offset + ((available_width as f32 - s.len() as f32) / 2.0).ceil() as usize
        };
        for (idx, c) in s
            .chars()
            .enumerate()
            .take_while(|(idx, _)| *idx < available_width)
        {
            self.get_tuxel(Position::Idx(idx + x_offset, y_offset))
                .set_content(c);
        }
        Ok(())
    }

    fn get_tuxel(&mut self, pos: Position) -> Tuxel {
        let (x, y) = self.rectangle.relative_idx(&pos);
        self.buf
            .get(y)
            .map(|row| row.get(x))
            .flatten()
            .map(|t| t.clone())
            .expect("using the buffer's rectangle should always yield a tuxel")
    }

    fn fill(&mut self, c: char) -> Result<()> {
        let (skipx, takex, skipy, takey) = if self.border {
            (
                1usize,
                self.rectangle.width() - 2,
                1usize,
                self.rectangle.height() - 2,
            )
        } else {
            (
                0usize,
                self.rectangle.width(),
                0usize,
                self.rectangle.height(),
            )
        };
        for row in self.buf.iter_mut().skip(skipy).take(takey) {
            for tuxel in row.iter_mut().skip(skipx).take(takex) {
                tuxel.set_content(c);
            }
        }
        Ok(())
    }

    fn draw_border(&mut self) -> Result<()> {
        let box_corner = boxy::Char::upper_left(boxy::Weight::Doubled);
        let box_horizontal = boxy::Char::horizontal(boxy::Weight::Doubled);
        let box_vertical = boxy::Char::vertical(boxy::Weight::Doubled);
        if self.buf.len() < 2 {
            // can only draw a border if there are at least two rows
            return Ok(());
        }

        // draw corners
        self.get_tuxel(Position::TopLeft)
            .set_content(box_corner.clone().into());
        self.get_tuxel(Position::TopRight)
            .set_content(box_corner.clone().rotate_cw(1).into());
        self.get_tuxel(Position::BottomRight)
            .set_content(box_corner.clone().rotate_cw(2).into());
        self.get_tuxel(Position::BottomLeft)
            .set_content(box_corner.clone().rotate_ccw(1).into());

        // draw non-corner top
        for tuxel in self
            .buf
            .iter()
            .nth(0)
            .expect("drawbuffer rows are always populated")
            .iter()
            .skip(1)
            .take(self.rectangle.width() - 2)
        {
            tuxel.clone().set_content(box_horizontal.clone().into());
        }

        // draw non-corner bottom
        for tuxel in self
            .buf
            .iter()
            .nth(self.rectangle.height() - 1)
            .expect("drawbuffer rows are always populated")
            .iter()
            .skip(1)
            .take(self.rectangle.width() - 2)
        {
            tuxel.clone().set_content(box_horizontal.clone().into());
        }

        // draw non-corner sides
        for row in self
            .buf
            .iter()
            // skip the first row
            .skip(1)
            // skip the last row
            .take(self.rectangle.height() - 2)
        {
            row.iter()
                .nth(0)
                .expect("drawbuffer rows are always populated")
                .clone()
                .set_content(box_vertical.clone().into());
            row.iter()
                .nth(self.rectangle.width() - 1)
                .expect("drawbuffer rows are always populated")
                .clone()
                .set_content(box_vertical.clone().into());
        }

        self.border = true;

        Ok(())
    }

    fn tuxel_is_active(&self, x: usize, y: usize) -> Result<bool> {
        Ok(self.buf[y][x].active())
    }

    fn tuxel_modifiers(&self, x: usize, y: usize) -> Result<Vec<Modifier>> {
        Ok(self.buf[y][x].modifiers())
    }

    fn tuxel_content(&self, x: usize, y: usize) -> Result<char> {
        Ok(self.buf[y][x].content())
    }
}

#[derive(Clone)]
pub(crate) struct DrawBuffer {
    inner: Arc<Mutex<DrawBufferInner>>,
    sender: Sender<Tuxel>,
}

impl DrawBuffer {
    pub(crate) fn new(
        sender: Sender<Tuxel>,
        rectangle: Rectangle,
        modifiers: SharedModifiers,
    ) -> Self {
        let mut buf: Vec<_> = Vec::with_capacity(rectangle.height());
        for _ in 0..rectangle.height() {
            let row: Vec<Tuxel> = Vec::with_capacity(rectangle.width());
            buf.push(row);
        }
        Self {
            inner: Arc::new(Mutex::new(DrawBufferInner {
                rectangle,
                border: false,
                buf,
                modifiers,
            })),
            sender,
        }
    }

    pub(crate) fn set_buf(&mut self, buf: Vec<Vec<Tuxel>>) -> Result<()> {
        self.lock().buf = buf;
        Ok(())
    }

    pub(crate) fn modify(&mut self, modifier: Modifier) {
        self.lock().modifiers.push(modifier)
    }

    pub(crate) fn draw_border(&mut self) -> Result<()> {
        self.lock().draw_border()
    }

    pub(crate) fn fill(&mut self, c: char) -> Result<()> {
        self.lock().fill(c)
    }

    pub(crate) fn write_left(&mut self, s: &str) -> Result<()> {
        self.lock().write_left(s)
    }

    pub(crate) fn write_right(&mut self, s: &str) -> Result<()> {
        self.lock().write_right(s)
    }

    pub(crate) fn write_center(&mut self, s: &str) -> Result<()> {
        self.lock().write_center(s)
    }

    fn tuxel_content(&self, x: usize, y: usize) -> Result<char> {
        self.lock().tuxel_content(x, y)
    }

    fn tuxel_is_active(&self, x: usize, y: usize) -> Result<bool> {
        self.lock().tuxel_is_active(x, y)
    }

    fn tuxel_modifiers(&self, x: usize, y: usize) -> Result<Vec<Modifier>> {
        self.lock().tuxel_modifiers(x, y)
    }
}

impl<'a> DrawBuffer {
    pub(crate) fn lock(&'a self) -> MutexGuard<'a, DrawBufferInner> {
        self.inner
            .as_ref()
            .lock()
            .expect("TODO: handle thread panicking better than this")
    }

    pub(crate) fn push(&mut self, t: Tuxel) -> DBTuxel {
        let mut inner = self.lock();
        let idx = t.idx();
        let buf_idx = Idx(idx.0 - inner.rectangle.x(), idx.1 - inner.rectangle.y(), 0);
        inner.buf.iter_mut().nth(buf_idx.1).expect("meow").push(t);
        DBTuxel {
            parent: self.clone(),
            idx,
            buf_idx,
        }
    }
}

pub(crate) struct DBTuxel {
    parent: DrawBuffer,
    idx: Idx,
    buf_idx: Idx,
}

impl DBTuxel {
    pub(crate) fn content(&self) -> Result<char> {
        self.parent.tuxel_content(self.buf_idx.0, self.buf_idx.1)
    }

    pub(crate) fn active(&self) -> Result<bool> {
        self.parent.tuxel_is_active(self.buf_idx.0, self.buf_idx.1)
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.buf_idx.0, self.buf_idx.1)
    }

    pub(crate) fn modifiers(&self) -> Result<Vec<Modifier>> {
        self.parent.tuxel_modifiers(self.buf_idx.0, self.buf_idx.1)
    }
}
