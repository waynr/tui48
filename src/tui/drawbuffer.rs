use std::sync::{Arc, Mutex, MutexGuard};

use super::error::Result;
use super::canvas::{Modifier, SharedModifiers};
use super::tuxel::Tuxel;
use super::geometry::{Position, Rectangle};

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
            self.get_tuxel(Position::Idx(offset + x, y))
                .set_content(c);
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
            self.get_tuxel(Position::Idx(x - offset, y))
                .set_content(c);
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
            tuxel
                .clone()
                .set_content(box_horizontal.clone().into());
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
            tuxel
                .clone()
                .set_content(box_horizontal.clone().into());
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
}

pub(crate) struct DrawBuffer {
    inner: Arc<Mutex<DrawBufferInner>>,
}

impl DrawBuffer {
    pub(crate) fn new(rectangle: Rectangle, buf: Vec<Vec<Tuxel>>, modifiers: SharedModifiers) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DrawBufferInner {
                rectangle,
                border: false,
                buf,
                modifiers,
            })),
        }
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
}

impl<'a> DrawBuffer {
    pub(crate) fn lock(&'a self) -> MutexGuard<'a, DrawBufferInner> {
        self.inner
            .as_ref()
            .lock()
            .expect("TODO: handle thread panicking better than this")
    }
}

