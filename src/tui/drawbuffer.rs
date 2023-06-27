use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use super::canvas::{Canvas, Modifier};
use super::colors::Rgb;
use super::error::{Result, TuiError};
use super::geometry::{Direction, Idx, Position, Rectangle};
use super::tuxel::Tuxel;

pub(crate) struct DrawBufferInner {
    pub(crate) rectangle: Rectangle,
    pub(crate) border: bool,
    pub(crate) buf: Vec<Vec<Tuxel>>,
    pub(crate) modifiers: Vec<Modifier>,
    pub(crate) canvas: Canvas,
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

    fn get_tuxel(&mut self, pos: Position) -> &mut Tuxel {
        let (x, y) = self.rectangle.relative_idx(&pos);
        self.buf
            .get_mut(y)
            .map(|row| row.get_mut(x))
            .flatten()
            .expect("using the buffer's rectangle should always yield a tuxel")
    }

    fn rectangle(&self) -> Rectangle {
        self.rectangle.clone()
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
            .iter_mut()
            .nth(0)
            .expect("drawbuffer rows are always populated")
            .iter_mut()
            .skip(1)
            .take(self.rectangle.width() - 2)
        {
            tuxel.set_content(box_horizontal.clone().into());
        }

        // draw non-corner bottom
        for tuxel in self
            .buf
            .iter_mut()
            .nth(self.rectangle.height() - 1)
            .expect("drawbuffer rows are always populated")
            .iter_mut()
            .skip(1)
            .take(self.rectangle.width() - 2)
        {
            tuxel.set_content(box_horizontal.clone().into());
        }

        // draw non-corner sides
        for row in self
            .buf
            .iter_mut()
            // skip the first row
            .skip(1)
            // skip the last row
            .take(self.rectangle.height() - 2)
        {
            row.iter_mut()
                .nth(0)
                .expect("drawbuffer rows are always populated")
                .set_content(box_vertical.clone().into());
            row.iter_mut()
                .nth(self.rectangle.width() - 1)
                .expect("drawbuffer rows are always populated")
                .set_content(box_vertical.clone().into());
        }

        self.border = true;

        Ok(())
    }

    fn translate(&mut self, magnitude: usize, dir: Direction) -> Result<()> {
        match dir {
            Direction::Left => {
                for t in self.buf.iter_mut().flatten() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    new_idx.0 = new_idx.0 - magnitude;
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
            Direction::Right => {
                for t in self.buf.iter_mut().flatten().rev() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    new_idx.0 = new_idx.0 + magnitude;
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
            Direction::Up => {
                for t in self.buf.iter_mut().flatten() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    new_idx.1 = new_idx.1 - magnitude;
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
            Direction::Down => {
                for t in self.buf.iter_mut().flatten().rev() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    new_idx.1 = new_idx.1 + magnitude;
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
        }
        self.rectangle.translate(magnitude, dir)?;
        Ok(())
    }
}

// Tuxel-querying methods.
impl DrawBufferInner {
    fn tuxel_is_active(&self, x: usize, y: usize) -> Result<bool> {
        Ok(self.buf[y][x].active())
    }

    fn tuxel_colors(&self, x: usize, y: usize) -> (Option<Rgb>, Option<Rgb>) {
        self.buf[y][x].colors()
    }

    fn tuxel_content(&self, x: usize, y: usize) -> Result<char> {
        Ok(self.buf[y][x].content())
    }
}

pub(crate) struct DrawBuffer {
    inner: Arc<Mutex<DrawBufferInner>>,
    sender: Sender<Tuxel>,
}

impl DrawBuffer {
    pub(crate) fn new(sender: Sender<Tuxel>, rectangle: Rectangle, canvas: Canvas) -> Self {
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
                modifiers: Vec::new(),
                canvas,
            })),
            sender,
        }
    }

    pub(crate) fn push(&mut self, t: Tuxel) -> DBTuxel {
        let mut inner = self.lock();
        let idx = t.idx();
        let buf_idx = Idx(idx.0 - inner.rectangle.x(), idx.1 - inner.rectangle.y(), 0);
        inner.buf.iter_mut().nth(buf_idx.1).expect("meow").push(t);
        DBTuxel {
            parent: self.inner.clone(),
            idx,
            buf_idx,
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

    pub(crate) fn rectangle(&self) -> Rectangle {
        self.lock().rectangle()
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

impl Drop for DrawBuffer {
    fn drop(&mut self) {
        for row in self.lock().buf.iter_mut() {
            while let Some(mut tuxel) = row.pop() {
                tuxel.clear();
                // can't do anything about send errors here -- we rely on the channel having the
                // necessary capacity and the Canvas outliving all DrawBuffers
                let _ = self.sender.send(tuxel);
            }
        }
    }
}

pub(crate) struct DBTuxel {
    parent: Arc<Mutex<DrawBufferInner>>,
    idx: Idx,
    buf_idx: Idx,
}

impl DBTuxel {
    fn lock(&self) -> MutexGuard<DrawBufferInner> {
        self.parent
            .lock()
            .expect("TODO: handle mutex lock errors more gracefully")
    }

    pub(crate) fn content(&self) -> Result<char> {
        self.lock().tuxel_content(self.buf_idx.0, self.buf_idx.1)
    }

    pub(crate) fn active(&self) -> Result<bool> {
        self.lock().tuxel_is_active(self.buf_idx.0, self.buf_idx.1)
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.idx.0, self.idx.1)
    }

    pub(crate) fn set_canvas_idx(&mut self, idx: &Idx) {
        self.idx = idx.clone()
    }

    pub(crate) fn colors(&self) -> (Option<Rgb>, Option<Rgb>) {
        let inner = self.lock();
        let colors = inner.tuxel_colors(self.buf_idx.x(), self.buf_idx.y());
        inner
            .modifiers
            .iter()
            .fold(colors, |cs, modifier| modifier.apply(cs))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    fn rectangle(x: usize, y: usize, z: usize, width: usize, height: usize) -> Rectangle {
        Rectangle(Idx(x, y, z), Bounds2D(width, height))
    }

    fn tuxel_buf_from_rectangle(rect: &Rectangle) -> Vec<Vec<Tuxel>> {
        let mut tuxels: Vec<Vec<Tuxel>> = Vec::new();
        for y in 0..rect.height() {
            let mut row: Vec<Tuxel> = Vec::new();
            for x in 0..rect.width() {
                let t = Tuxel::new(Idx(x, y, 0));
                row.push(t);
            }
            tuxels.push(row);
        }

        tuxels
    }

    #[rstest]
    #[case::base(rectangle(0, 0, 0, 5, 5))]
    #[case::asymmetric(rectangle(0, 0, 0, 274, 75))]
    #[case::ignore_index(rectangle(10, 10, 0, 10, 10))]
    fn new_draw_buffer(#[case] rect: Rectangle) -> Result<()> {
        let tuxels = tuxel_buf_from_rectangle(&rect);
        let (sender, receiver) = channel();
        let mut dbuf = DrawBuffer::new(sender.clone(), rect.clone(), Vec::new());
        dbuf.set_buf(tuxels)?;
        {
            let inner = dbuf.lock();
            assert_eq!(inner.buf.len(), rect.height());
            for row in &inner.buf {
                assert_eq!(row.len(), rect.width());
            }
        }
        drop(dbuf);
        let mut count = 0;
        loop {
            match receiver.try_recv() {
                Ok(_) => count = count + 1,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    unreachable!();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
            }
        }
        assert_eq!(count, rect.width() * rect.height());
        Ok(())
    }
}
