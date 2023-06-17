use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::Result;

/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Default)]
pub(crate) struct Idx(pub usize, pub usize, pub usize);

#[derive(Clone, Default)]
pub(crate) struct Bounds2D(pub usize, pub usize);

#[derive(Clone, Default)]
pub(crate) struct Rectangle(pub Idx, pub Bounds2D);

impl Rectangle {
    fn width(&self) -> usize {
        self.1 .0
    }

    fn height(&self) -> usize {
        self.1 .1
    }

    fn x(&self) -> usize {
        self.0 .0
    }

    fn y(&self) -> usize {
        self.0 .1
    }

    fn relative_idx(&self, pos: &Position) -> (usize, usize) {
        match pos {
            Position::TopLeft => (0, 0),
            Position::TopRight => (self.width() - 1, 0),
            Position::BottomLeft => (0, self.height() - 1),
            Position::BottomRight => (self.width() - 1, self.height() - 1),
            Position::Idx(x, y) => (*x, *y),
        }
    }

    pub(crate) fn extents(&self) -> (usize, usize) {
        (self.0 .0 + self.1 .0, self.0 .1 + self.1 .1)
    }
}

pub(crate) enum Position {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Idx(usize, usize),
}

/// A 2d grid of `Cell`s.
pub(crate) struct Canvas {
    grid: Vec<Vec<Stack>>,
    rectangle: Rectangle,
    receiver: Receiver<Stack>,
    sender: Sender<Stack>,
}

impl Canvas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        let rectangle = Rectangle(Idx(0, 0, 0), Bounds2D(width, height));
        let mut grid: Vec<Vec<Stack>> = Vec::with_capacity(height);
        for y in 0..height {
            let mut row: Vec<Stack> = Vec::with_capacity(width);
            for x in 0..width {
                row.push(Stack::new(x, y));
            }
            grid.push(row);
        }
        let (sender, receiver) = channel();
        let mut s = Self {
            grid,
            rectangle,
            sender,
            receiver,
        };
        s.draw_all().expect("enqueuing entire canvas rerender");

        s
    }

    pub(crate) fn get_draw_buffer(&mut self, r: Rectangle) -> Result<DrawBuffer> {
        let mut buf: Vec<_> = Vec::with_capacity(r.height());
        let modifiers = SharedModifiers::default();
        for _ in 0..r.height() {
            let row: Vec<Tuxel> = Vec::with_capacity(r.width());
            buf.push(row);
        }
        for (buf_y, (y, row)) in self
            .grid
            .iter_mut()
            .enumerate()
            .skip(r.y())
            .take(r.height())
            .enumerate()
        {
            let buf_row = &mut buf[buf_y];
            for (x, cellstack) in row.iter_mut().enumerate().skip(r.x()).take(r.width()) {
                let canvas_idx = Idx(x, y, r.0 .2);
                buf_row.push(cellstack.acquire(canvas_idx, modifiers.clone())?);
            }
        }
        let dbuf = DrawBuffer::new(r.clone(), buf, modifiers);
        Ok(dbuf)
    }

    pub(crate) fn get_layer(&mut self, z: usize) -> Result<DrawBuffer> {
        self.get_draw_buffer(Rectangle(Idx(0, 0, z), self.rectangle.1.clone()))
    }

    pub(crate) fn draw_all(&mut self) -> Result<()> {
        for row in self.grid.iter() {
            for stack in row.iter() {
                self.sender.send(stack.clone())?
            }
        }
        Ok(())
    }

    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.rectangle.1 .0, self.rectangle.1 .1)
    }
}

impl Iterator for &Canvas {
    type Item = Result<Tuxel>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.receiver.try_recv() {
                Ok(stack) => match stack.top() {
                    Ok(Some(tuxel)) => return Some(Ok(tuxel)),
                    _ => continue,
                },
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    unreachable!();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            }
        }
    }
}

/// A stack of `Cells`. Enables z-ordering of elements with occlusion and update detection. Tuxels
/// are wrapped in a Arc<Mutex<_>> to allow them to be referenced by the higher level Widget
/// abstraction at the same time.
#[derive(Default)]
struct StackInner {
    cells: [Tuxel; 8],
}

#[derive(Clone, Default)]
pub(crate) struct Stack {
    inner: Arc<Mutex<StackInner>>,
}

impl Stack {
    fn new(x: usize, y: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StackInner {
                cells: [
                    Tuxel::new(Idx(x, y, 0)),
                    Tuxel::new(Idx(x, y, 1)),
                    Tuxel::new(Idx(x, y, 2)),
                    Tuxel::new(Idx(x, y, 3)),
                    Tuxel::new(Idx(x, y, 4)),
                    Tuxel::new(Idx(x, y, 5)),
                    Tuxel::new(Idx(x, y, 6)),
                    Tuxel::new(Idx(x, y, 7)),
                ],
            })),
        }
    }

    fn acquire(&mut self, idx: Idx, shared_modifiers: SharedModifiers) -> Result<Tuxel> {
        let clone = self.inner.clone();
        let mut inner = clone.lock().expect("lock unexpectedly poisoned");
        let tuxel = &mut inner.cells[idx.2];
        tuxel.clone().lock().inner.shared_modifiers = shared_modifiers;
        Ok(tuxel.clone())
    }

    fn top(&self) -> Result<Option<Tuxel>> {
        Ok(self
            .inner
            .lock()
            .expect("TODO")
            .cells
            // low-index elements of a stack are below high-index elements. we want to find the
            // first active tuxel on top of the stack so we iterate over elements in reverse
            .iter_mut()
            .rev()
            .find(|t| t.lock().active())
            .map(|s| s.clone()))
    }
}

#[derive(Clone, Default)]
struct TuxelInner {
    active: bool,
    content: char,
    idx: Idx,
    modifiers: Vec<Modifier>,
    shared_modifiers: SharedModifiers,
}

#[derive(Clone, Default)]
pub(crate) struct Tuxel {
    inner: Arc<Mutex<TuxelInner>>,
}

pub(crate) struct TuxelGuard<'a> {
    inner: MutexGuard<'a, TuxelInner>,
}

impl<'a> TuxelGuard<'a> {
    pub(crate) fn set_content(&mut self, c: char) {
        self.inner.active = true;
        self.inner.content = c;
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.inner.idx.0, self.inner.idx.1)
    }

    pub(crate) fn modifiers(&self) -> Vec<Modifier> {
        let parent_modifiers = &mut self.inner.shared_modifiers.lock();
        let mut modifiers: Vec<Modifier> = self.inner.modifiers.clone();
        parent_modifiers.append(&mut modifiers);
        parent_modifiers.to_vec()
    }

    fn clear(&mut self) {
        self.inner.content = ' ';
        self.inner.modifiers.clear();
    }

    pub(crate) fn active(&self) -> bool {
        self.inner.active
    }
}

impl std::fmt::Display for TuxelGuard<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.content)?;
        Ok(())
    }
}

impl std::fmt::Display for Tuxel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.clone().lock().fmt(f)
    }
}

impl<'a> Tuxel {
    fn new(idx: Idx) -> Self {
        Tuxel {
            inner: Arc::new(Mutex::new(TuxelInner {
                // use radioactive symbol to indicate user hasn't set a value for this Tuxel.
                //content: '\u{2622}',
                //content: '\u{2566}',
                active: false,
                content: 'x',
                idx,
                modifiers: Vec::new(),
                shared_modifiers: SharedModifiers::default(),
            })),
        }
    }

    pub(crate) fn lock(&'a self) -> TuxelGuard<'a> {
        TuxelGuard {
            inner: self
                .inner
                .lock()
                .expect("TODO: handle thread panicking better than this"),
        }
    }
}

#[derive(Default)]
struct DrawBufferInner {
    rectangle: Rectangle,
    border: bool,
    buf: Vec<Vec<Tuxel>>,
    modifiers: SharedModifiers,
}

impl Drop for DrawBufferInner {
    fn drop(&mut self) {
        for row in self.buf.iter_mut() {
            for mut tguard in row.iter_mut().map(|t| t.lock()) {
                tguard.clear();
                tguard.inner.shared_modifiers = SharedModifiers::default();
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
                .lock()
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
                .lock()
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
                .lock()
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
                tuxel.lock().set_content(c);
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
            .lock()
            .set_content(box_corner.clone().into());
        self.get_tuxel(Position::TopRight)
            .lock()
            .set_content(box_corner.clone().rotate_cw(1).into());
        self.get_tuxel(Position::BottomRight)
            .lock()
            .set_content(box_corner.clone().rotate_cw(2).into());
        self.get_tuxel(Position::BottomLeft)
            .lock()
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
                .lock()
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
                .lock()
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
                .lock()
                .set_content(box_vertical.clone().into());
            row.iter()
                .nth(self.rectangle.width() - 1)
                .expect("drawbuffer rows are always populated")
                .clone()
                .lock()
                .set_content(box_vertical.clone().into());
        }

        self.border = true;

        Ok(())
    }
}

#[derive(Clone, Default)]
pub(crate) struct DrawBuffer {
    inner: Arc<Mutex<DrawBufferInner>>,
}

impl DrawBuffer {
    fn new(rectangle: Rectangle, buf: Vec<Vec<Tuxel>>, modifiers: SharedModifiers) -> Self {
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
    fn lock(&'a self) -> MutexGuard<'a, DrawBufferInner> {
        self.inner
            .as_ref()
            .lock()
            .expect("TODO: handle thread panicking better than this")
    }
}

#[derive(Clone)]
pub(crate) enum Modifier {
    ForegroundColor(u8, u8, u8),
    BackgroundColor(u8, u8, u8),
    Bold,
}

#[derive(Clone, Default)]
struct SharedModifiers {
    inner: Arc<Mutex<Vec<Modifier>>>,
}

impl SharedModifiers {
    fn lock(&self) -> MutexGuard<Vec<Modifier>> {
        self.inner
            .lock()
            .expect("TODO: handle thread panicking better than this")
    }

    fn push(&self, m: Modifier) {
        self.lock().push(m)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case::base((5, 5))]
    #[case::realistic((274, 75))]
    //#[case::toobig((1000, 1000))]
    fn canvas_size(#[case] dims: (usize, usize)) {
        let canvas = Canvas::new(dims.0, dims.1);
        assert_eq!(canvas.grid.len(), dims.1);
        for row in &canvas.grid {
            assert_eq!(row.len(), dims.0);
        }
    }

    #[rstest]
    #[case::base((5, 5))]
    #[case::realistic((274, 75))]
    fn get_layer_validate_draw_buffer_size(#[case] dims: (usize, usize)) {
        let mut canvas = Canvas::new(dims.0, dims.1);
        let result = canvas.get_layer(0);
        assert!(result.is_ok());
        let buffer = result.unwrap();
        let inner = buffer.inner.lock().unwrap();
        assert_eq!(inner.buf.len(), dims.1);
        for row in &inner.buf {
            assert_eq!(row.len(), dims.0);
        }
    }

    fn rectangle(x: usize, y: usize, z: usize, width: usize, height: usize) -> Rectangle {
        Rectangle(Idx(x, y, z), Bounds2D(width, height))
    }

    #[rstest]
    #[case::base((5, 5), rectangle(0, 0, 0, 5, 5))]
    #[case::realistic((274, 75), rectangle(0, 0, 0, 274, 75))]
    #[case::realistic_smaller_buffer((274, 75), rectangle(10, 10, 0, 10, 10))]
    fn validate_get_draw_buffer(
        #[case] canvas_dims: (usize, usize),
        #[case] rect: Rectangle,
    ) -> Result<()> {
        let mut canvas = Canvas::new(canvas_dims.0, canvas_dims.1);
        let buffer = canvas.get_draw_buffer(rect.clone())?;

        let inner = buffer.inner.lock().unwrap();
        assert_eq!(
            inner.buf.len(),
            rect.height(),
            "validating height of draw buffer"
        );
        for row in &inner.buf {
            assert_eq!(row.len(), rect.width(), "validating width of draw buffer");
        }

        Ok(())
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
        let buf = DrawBuffer::new(rect.clone(), tuxels, SharedModifiers::default());
        let inner = buf.inner.lock().unwrap();
        assert_eq!(inner.buf.len(), rect.height());
        for row in &inner.buf {
            assert_eq!(row.len(), rect.width());
        }
        Ok(())
    }
}
