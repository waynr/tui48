use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::Result;
use crate::tui::tuxel::Tuxel;
use crate::tui::drawbuffer::DrawBuffer;
use crate::tui::{Idx, Bounds2D, Rectangle};

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
        let inner = clone.lock().expect("lock unexpectedly poisoned");
        let tuxel = inner.cells[idx.2].clone();
        tuxel.set_shared_modifiers(shared_modifiers.clone());
        Ok(tuxel)
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
            .find(|t| t.active())
            .map(|s| s.clone()))
    }
}

#[derive(Clone)]
pub(crate) enum Modifier {
    ForegroundColor(u8, u8, u8),
    BackgroundColor(u8, u8, u8),
    Bold,
}

#[derive(Clone, Default)]
pub(crate) struct SharedModifiers {
    inner: Arc<Mutex<Vec<Modifier>>>,
}

impl SharedModifiers {
    pub(crate) fn lock(&self) -> MutexGuard<Vec<Modifier>> {
        self.inner
            .lock()
            .expect("TODO: handle thread panicking better than this")
    }

    pub fn push(&self, m: Modifier) {
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
