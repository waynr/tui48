use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use super::canvas::{Canvas, Modifier};
use super::colors::Rgb;
use super::error::{InnerError, Result};
use super::geometry::{Direction, Idx, Position, Rectangle};
use super::tuxel::Tuxel;

pub(crate) struct DrawBufferInner {
    pub(crate) rectangle: Rectangle,
    pub(crate) border: bool,
    pub(crate) buf: Vec<Vec<Tuxel>>,
    pub(crate) modifiers: Vec<Modifier>,
    pub(crate) canvas: Canvas,
}

impl std::fmt::Display for DrawBufferInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in self.buf.iter() {
            write!(
                f,
                "{}\n",
                row.iter().map(|t| t.content()).collect::<String>()
            )?
        }
        Ok(())
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
            self.get_tuxel_mut(Position::Coordinates(offset + x, y))?
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
            self.get_tuxel_mut(Position::Coordinates(x - offset, y))?
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
            self.get_tuxel_mut(Position::Coordinates(idx + x_offset, y_offset))?
                .set_content(c);
        }
        Ok(())
    }

    #[inline(always)]
    fn get_tuxel_mut(&mut self, pos: Position) -> Result<&mut Tuxel> {

        let (x, y) = self.rectangle.relative_idx(&pos);
        log::trace!("get_tuxel_mut: {0}, {1}", x, y);
        let t = self
            .buf
            .get_mut(y)
            .ok_or(InnerError::OutOfBoundsY(y))?
            .get_mut(x)
            .ok_or(InnerError::OutOfBoundsX(x))?;
        Ok(t)
    }

    #[inline(always)]
    fn get_tuxel(&self, pos: Position) -> Result<&Tuxel> {
        let (x, y) = self.rectangle.relative_idx(&pos);
        let t = self
            .buf
            .get(y)
            .ok_or(InnerError::OutOfBoundsY(y))?
            .get(x)
            .ok_or(InnerError::OutOfBoundsX(x))?;
        Ok(t)
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
        self.get_tuxel_mut(Position::TopLeft)?
            .set_content(box_corner.clone().into());
        self.get_tuxel_mut(Position::TopRight)?
            .set_content(box_corner.clone().rotate_cw(1).into());
        self.get_tuxel_mut(Position::BottomRight)?
            .set_content(box_corner.clone().rotate_cw(2).into());
        self.get_tuxel_mut(Position::BottomLeft)?
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

    fn switch_layer(&mut self, zdx: usize) -> Result<()> {
        if self.rectangle.0 .2 == zdx {
            // shh, don't tell the caller that we didn't have to do anything
            return Ok(());
        }
        log::trace!("switching layer from {0} to {1}", self.rectangle.z(), zdx);

        let old = self.rectangle.clone();
        self.rectangle.0 .2 = zdx;

        self.canvas.swap_rectangles(&self.rectangle, &old)?;

        for tuxel in self.buf.iter_mut().map(|v| v.iter_mut()).flatten() {
            let mut idx = tuxel.idx();
            idx.2 = zdx;
            tuxel.set_idx(&idx);
        }
        Ok(())
    }

    fn translate(&mut self, dir: Direction) -> Result<()> {
        self.rectangle.translate(1, &dir)?;
        let canvas_bounds = self.canvas.bounds();
        log::trace!("translating DrawBuffer {}", dir);
        match dir {
            Direction::Left => {
                for t in self.buf.iter_mut().flatten() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    if new_idx.0 > 0 {
                        new_idx.0 -= 1
                    } else {
                        return Err(
                            InnerError::DrawBufferTranslationFailed(String::from("")).into()
                        );
                    }
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
            Direction::Right => {
                for t in self.buf.iter_mut().flatten().rev() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    if new_idx.0 < canvas_bounds.width() {
                        new_idx.0 += 1
                    } else {
                        return Err(
                            InnerError::DrawBufferTranslationFailed(String::from("")).into()
                        );
                    }
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
            Direction::Up => {
                for t in self.buf.iter_mut().flatten() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    if new_idx.1 > 0 {
                        new_idx.1 -= 1
                    } else {
                        return Err(
                            InnerError::DrawBufferTranslationFailed(String::from("")).into()
                        );
                    }

                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
            Direction::Down => {
                for t in self.buf.iter_mut().flatten().rev() {
                    let current_idx = t.idx();
                    let mut new_idx = current_idx.clone();
                    if new_idx.1 < canvas_bounds.height() {
                        new_idx.1 += 1;
                    } else {
                        return Err(
                            InnerError::DrawBufferTranslationFailed(String::from("")).into()
                        );
                    }
                    self.canvas.swap_tuxels(current_idx, new_idx.clone())?;
                    t.set_idx(&new_idx);
                }
            }
        }
        self.canvas.reclaim()?;
        Ok(())
    }
}

// Tuxel-querying methods.
impl DrawBufferInner {
    fn tuxel_is_active(&self, x: usize, y: usize) -> Result<bool> {
        Ok(self.get_tuxel(Position::Coordinates(x, y))?.active())
    }

    fn tuxel_colors(&self, x: usize, y: usize) -> (Option<Rgb>, Option<Rgb>) {
        self.buf[y][x].colors()
    }

    fn tuxel_content(&self, x: usize, y: usize) -> Result<char> {
        Ok(self.get_tuxel(Position::Coordinates(x, y))?.content())
    }
}

pub(crate) struct DrawBuffer {
    inner: Arc<Mutex<DrawBufferInner>>,
    sender: Sender<Tuxel>,
}

impl std::fmt::Display for DrawBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.lock().fmt(f)
    }
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
        let canvas_idx = t.idx();
        let buf_idx = Idx(
            canvas_idx.0 - inner.rectangle.x(),
            canvas_idx.1 - inner.rectangle.y(),
            0,
        );
        inner.buf.iter_mut().nth(buf_idx.1).expect("meow").push(t);
        DBTuxel {
            parent: self.inner.clone(),
            canvas_idx,
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

    pub(crate) fn translate(&self, dir: Direction) -> Result<()> {
        self.lock().translate(dir)
    }

    pub(crate) fn switch_layer(&self, zdx: usize) -> Result<()> {
        self.lock().switch_layer(zdx)
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
        let mut inner = self.lock();
        for row in inner.buf.iter_mut() {
            while let Some(mut tuxel) = row.pop() {
                tuxel.clear();
                // can't do anything about send errors here -- we rely on the channel having the
                // necessary capacity and the Canvas outliving all DrawBuffers
                let _ = self.sender.send(tuxel);
            }
        }
        let _ = inner.canvas.reclaim();
    }
}

pub(crate) struct DBTuxel {
    parent: Arc<Mutex<DrawBufferInner>>,
    canvas_idx: Idx,
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
        (self.canvas_idx.0, self.canvas_idx.1)
    }

    pub(crate) fn set_canvas_idx(&mut self, new_idx: &Idx) -> Result<()> {
        self.canvas_idx = new_idx.clone();
        // NOTE: in the early stages of development the only case i can think of where this would
        // block is when swapping tuxels for a specific draw buffer. since the actual high-level
        // operation in such cases requires the DrawBufferInner corresponding to this DBTuxel to
        // already be locked. in those cases it is the DrawBuffer's responsibility to ensure tuxel
        // canvas indices are set properly.
        //
        // but since there may come a time in the near future where drawbuffers and canvases are
        // managed by separate threads there is the possibility of a coincidental and ephemeral
        // (rather than deadlocking) simultaneous lock -- because of that we should retry a few
        // times
        let retry_count = 0usize;
        let max_retries = 1usize;
        let mut dbi = match (retry_count..max_retries).into_iter().find_map(
            |i| -> Option<MutexGuard<DrawBufferInner>> {
                match self.parent.try_lock() {
                    Ok(guard) => Some(guard),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        std::thread::sleep(std::time::Duration::from_millis(i as u64 * 5));
                        None
                    }
                    Err(std::sync::TryLockError::Poisoned(p_err)) => {
                        let recovered = p_err.into_inner();
                        // TODO: what kind of recovery routines should be run on recovered
                        // drawbuffers? should probably be doing this everywhere we attempt to lock
                        // mutexes... :thinkies: mutices???
                        Some(recovered)
                    }
                }
            },
        ) {
            Some(g) => g,
            None => {
                return Err(
                    InnerError::ExceedRetryLimitForLockingDrawBuffer(String::from(
                        "setting canvas index for drawbuffer-owned tuxel",
                    ))
                    .into(),
                )
            }
        };
        let t = dbi.get_tuxel_mut(self.buf_idx.clone().into())?;
        t.set_idx(new_idx);
        Ok(())
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
    use std::sync::mpsc::channel;
    use std::sync::mpsc::Receiver;

    use super::*;
    use rstest::*;

    use super::super::geometry::Bounds2D;

    fn rectangle(x: usize, y: usize, z: usize, width: usize, height: usize) -> Rectangle {
        Rectangle(Idx(x, y, z), Bounds2D(width, height))
    }

    fn verify_messages_sent(receiver: &Receiver<Tuxel>, expected: usize) {
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
        assert_eq!(count, expected);
    }

    // #[case::<CASENAME>(
    //     rectangle(<X>, <Y>, <Z>, <WIDTH>, <HEIGHT>),
    // )]
    #[rstest]
    #[case::base(rectangle(0, 0, 0, 5, 5))]
    #[case::asymmetric(rectangle(0, 0, 0, 274, 75))]
    #[case::ignore_index(rectangle(10, 10, 0, 10, 10))]
    #[case::zero_width_draw_buffer(rectangle(0, 0, 0, 0, 10))]
    #[case::zero_height_draw_buffer(rectangle(0, 0, 0, 10, 0))]
    fn new_draw_buffer(#[case] rect: Rectangle) -> Result<()> {
        let canvas = Canvas::new(rect.width() * 2, rect.height() * 2);
        let (sender, receiver) = channel();
        let mut dbuf = canvas.get_draw_buffer(rect.clone())?;
        dbuf.sender = sender.clone();
        {
            let inner = dbuf.lock();
            assert_eq!(inner.buf.len(), rect.height());
            for row in &inner.buf {
                assert_eq!(row.len(), rect.width());
            }
        }

        verify_messages_sent(&receiver, 0);
        drop(dbuf);
        verify_messages_sent(&receiver, rect.width() * rect.height());

        Ok(())
    }

    #[rstest]
    // #[case::base(
    //      canvas_width, canvas_height,
    //      rectangle(<X>, <Y>, <Z>, <WIDTH>, <HEIGHT>)
    //      target_layer,
    // )]
    #[case::base_same_layer(rectangle(0, 0, 0, 5, 5), 1)]
    #[case::one_layer_down(rectangle(0, 0, 1, 5, 5), 0)]
    #[case::one_layer_up(rectangle(0, 0, 1, 5, 5), 2)]
    fn drawbuffer_switch_layer(#[case] rect: Rectangle, #[case] target_layer: usize) -> Result<()> {
        let canvas = Canvas::new(rect.width() * 2, rect.height() * 2);
        let dbuf = canvas.get_draw_buffer(rect.clone())?;
        for _ in 0..10 {
            dbuf.switch_layer(target_layer)?;
            dbuf.switch_layer(rect.0 .2)?;
        }
        Ok(())
    }

    #[rstest]
    // #[case::base(
    //      canvas_width, canvas_height,
    //      rectangle(<X>, <Y>, <Z>, <WIDTH>, <HEIGHT>)
    // )]
    #[case::rectangle_larger_than_canvas(100, 100, rectangle(0, 0, 0, 200, 105))]
    #[case::draw_buffer_far_outside_canvas_y_bounds(100, 100, rectangle(0, 1000, 0, 1, 1))]
    #[case::draw_buffer_far_outside_canvas_x_bounds(100, 100, rectangle(1000, 0, 0, 1, 1))]
    // TODO: the following test case causes `cargo test` to hang indefinitely. the hang is somehow
    // related to requesting a layer index that's not currently supported by the tui::canvas::Stack
    // type
    //#[case::draw_buffer_far_outside_canvas_z_bounds(100, 100, rectangle(0, 0, 8, 1, 1))]
    #[case::draw_buffer_overlaps_on_right_edge(100, 100, rectangle(50, 50, 0, 1, 100))]
    #[case::draw_buffer_overlaps_on_bottom_edge(100, 100, rectangle(50, 50, 0, 100, 1))]
    // TODO: the following two test cases aren't possible since we don't support negative coordinates
    // (maybe we should though)
    //#[case::draw_buffer_overlaps_on_left_edge(100, 100, rectangle(0, 0, 0, 1, 100))]
    //#[case::draw_buffer_overlaps_on_top_edge(100, 100, rectangle(0, 0, 0, 100, 1))]
    fn drawbuffer_switch_layer_error(
        #[case] canvas_height: usize,
        #[case] canvas_width: usize,
        #[case] rect: Rectangle,
    ) -> Result<()> {
        let canvas = Canvas::new(canvas_height, canvas_width);
        let r = canvas.get_draw_buffer(rect.clone());
        assert!(r.is_err());
        Ok(())
    }
}
