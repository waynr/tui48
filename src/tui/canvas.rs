use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::{Error, Result};

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
        for _ in 0..height {
            let mut row: Vec<Stack> = Vec::with_capacity(width);
            for _ in 0..width {
                row.push(Stack::default());
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
        let buf = DrawBuffer::new(r.clone());
        for (buf_y, (y, row)) in self
            .grid
            .iter_mut()
            .enumerate()
            .skip(r.y())
            .take(r.height())
            .enumerate()
        {
            for (buf_x, (x, cellstack)) in row
                .iter_mut()
                .enumerate()
                .skip(r.x())
                .take(r.width())
                .enumerate()
            {
                let canvas_idx = Idx(x, y, r.0 .2);
                let buf_idx = Idx(buf_x, buf_y, r.0 .2);
                let tuxel = cellstack.acquire(canvas_idx, Some(buf.clone()))?;
                buf.insert(&buf_idx, tuxel)?;
            }
        }
        Ok(buf)
    }

    pub(crate) fn get_layer(&mut self, z: usize) -> Result<DrawBuffer> {
        self.get_draw_buffer(Rectangle(Idx(0, 0, z), self.rectangle.1.clone()))
    }

    pub(crate) fn draw_all(&mut self) -> Result<()> {
        for (y, row) in self.grid.iter().enumerate() {
            for (x, stack) in row.iter().enumerate() {
                self.sender.send(stack.clone())?
            }
        }
        Ok(())
    }

    fn translate_tuxels(&mut self, ts: Vec<Arc<Mutex<Tuxel>>>) -> Result<()> {
        Err(String::from("not implemented").into())
    }
}

impl Iterator for &Canvas {
    type Item = Result<Tuxel>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.try_recv() {
            Ok(stack) => Some(stack.top()),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                unreachable!();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        }
    }
}

/// A stack of `Cells`. Enables z-ordering of elements with occlusion and update detection. Tuxels
/// are wrapped in a Arc<Mutex<_>> to allow them to be referenced by the higher level Widget
/// abstraction at the same time.
#[derive(Default)]
struct StackInner {
    cells: [Tuxel; 8],
    top_index: usize,
}

#[derive(Clone, Default)]
struct Stack {
    inner: Arc<Mutex<StackInner>>,
}

impl Stack {
    fn acquire(&mut self, idx: Idx, buf: Option<DrawBuffer>) -> Result<Tuxel> {
        let clone = self.inner.clone();
        let mut inner = clone.lock().expect("lock unexpectedly poisoned");
        let tuxel = &mut inner.cells[idx.2];
        match tuxel.inner {
            Some(_) => Err(String::from("tuxel already occupied!").into()),
            None => {
                let new = Tuxel::new(idx, buf);
                let _ = tuxel.insert(new.clone());
                Ok(new)
            }
        }
    }

    fn top(&self) -> Result<Tuxel> {
        let cloned = self.inner.clone();
        let readable = cloned.lock().expect("lock unexpectedly poisoned");

        Ok(readable
            .cells
            .iter()
            .rev()
            .find(|t| t.inner.is_some())
            .map_or_else(
                || {
                    readable
                        .cells
                        .first()
                        .expect("Stack is always populated")
                        .clone()
                },
                |s| s.clone(),
            ))
    }
}

struct TuxelInner {
    content: char,
    idx: Idx,
    before_modifiers: Vec<Modifier>,
    after_modifiers: Vec<Modifier>,
    partof: Option<DrawBuffer>,
}

#[derive(Clone, Default)]
pub(crate) struct Tuxel {
    inner: Option<Arc<Mutex<TuxelInner>>>,
}

pub(crate) struct TuxelGuard<'a> {
    inner: MutexGuard<'a, TuxelInner>,
}

impl<'a> TuxelGuard<'a> {
    pub(crate) fn set_content(&mut self, c: char) {
        self.inner.content = c;
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.inner.idx.0, self.inner.idx.1)
    }

    pub(crate) fn before(&self) -> Vec<Modifier> {
        let mut parent_modifiers: Vec<Modifier> = if let Some(parent) = &self.inner.partof {
            parent
                .inner
                .lock()
                .expect("TOOD: handle thread panicking better than this")
                .before_modifiers
                .iter()
                .map(|m| m.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut modifiers: Vec<Modifier> = self.inner.before_modifiers.clone();
        parent_modifiers.append(&mut modifiers);
        parent_modifiers
    }

    pub(crate) fn after(&self) -> Vec<Modifier> {
        let mut parent_modifiers: Vec<Modifier> = if let Some(parent) = &self.inner.partof {
            parent
                .inner
                .lock()
                .expect("TODO: handle thread panicking better than this")
                .after_modifiers
                .iter()
                .map(|m| m.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut modifiers: Vec<Modifier> = self.inner.after_modifiers.clone();
        parent_modifiers.append(&mut modifiers);
        parent_modifiers
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
        if let Some(guard) = self.clone().lock() {
            guard.fmt(f)?;
        }
        Ok(())
    }
}

impl<'a> Tuxel {
    fn new(idx: Idx, buf: Option<DrawBuffer>) -> Self {
        Tuxel {
            inner: Some(Arc::new(Mutex::new(TuxelInner {
                // use radioactive symbol to indicate user hasn't set a value for this Tuxel.
                //content: '\u{2622}',
                //content: '\u{2566}',
                content: 'x',
                idx,
                before_modifiers: Vec::new(),
                after_modifiers: Vec::new(),
                partof: buf,
            }))),
        }
    }

    fn insert(&mut self, other: Tuxel) {
        match other.inner {
            Some(i) => self.inner.insert(i),
            None => return,
        };
    }

    pub(crate) fn lock(&'a mut self) -> Option<TuxelGuard<'a>> {
        self.inner
            .as_ref()
            .map(|v| v.lock())
            .transpose()
            .expect("")
            .map(|v| TuxelGuard { inner: v })
    }
}

#[derive(Default)]
struct DrawBufferInner {
    rectangle: Rectangle,
    border: bool,
    buf: Vec<Vec<Tuxel>>,
    before_modifiers: Vec<Modifier>,
    after_modifiers: Vec<Modifier>,
}

#[derive(Clone, Default)]
pub(crate) struct DrawBuffer {
    inner: Arc<Mutex<DrawBufferInner>>,
}

impl DrawBuffer {
    fn new(rectangle: Rectangle) -> Self {
        let mut buf: Vec<_> = Vec::with_capacity(rectangle.height());
        for _ in 0..rectangle.height() {
            let mut row: Vec<Tuxel> = Vec::with_capacity(rectangle.width());
            for _ in 0..rectangle.width() {
                row.push(Tuxel::default())
            }
            buf.push(row);
        }
        Self {
            inner: Arc::new(Mutex::new(DrawBufferInner {
                rectangle,
                border: false,
                buf,
                before_modifiers: Vec::new(),
                after_modifiers: Vec::new(),
            })),
        }
    }

    fn insert(&self, idx: &Idx, tuxel: Tuxel) -> Result<()> {
        let new_inner = tuxel
            .inner
            .expect("tuxel to be inserted must always be some");
        let mut inner = self
            .inner
            .lock()
            .expect("TODO: handle thread panicking better than this");
        let current = inner
            .buf
            .get_mut(idx.1)
            .ok_or(Error::OutOfBoundsY(idx.1))?
            .get_mut(idx.0)
            .ok_or(Error::OutOfBoundsX(idx.0))?;
        match current.inner {
            Some(_) => Err(String::from("DrawBuffer tuxel slot already occupied").into()),
            None => {
                let _ = current.inner.insert(new_inner);
                Ok(())
            }
        }
    }

    pub(crate) fn modify_before(&mut self, modifier: Modifier) {
        let mut inner = self
            .inner
            .lock()
            .expect("TODO: handle thread panicking better than this");
        inner.before_modifiers.push(modifier)
    }

    pub(crate) fn modify_after(&mut self, modifier: Modifier) {
        let mut inner = self
            .inner
            .lock()
            .expect("TODO: handle thread panicking better than this");
        inner.after_modifiers.push(modifier)
    }

    pub(crate) fn draw_border(&mut self) -> Result<()> {
        Ok(())
    }

    pub(crate) fn fill(&mut self, c: char) -> Result<()> {
        let inner = self.inner.clone();
        let mut locked = inner
            .lock()
            .expect("TODO: handle thread panicking better than this");
        let (skipx, takex, skipy, takey) = if locked.border {
            (
                1usize,
                locked.rectangle.1 .0 - 2,
                1usize,
                locked.rectangle.1 .1 - 2,
            )
        } else {
            (0usize, locked.rectangle.1 .0, 0usize, locked.rectangle.1 .1)
        };
        for row in locked.buf.iter_mut().skip(skipy).take(takey) {
            for tuxel in row.iter_mut().skip(skipx).take(takex) {
                if let Some(mut tuxel) = tuxel.lock() {
                    tuxel.set_content(c);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) enum Modifier {
    ForegroundColor(u8, u8, u8),
    BackgroundColor(u8, u8, u8),
    Bold,
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

    #[rstest]
    #[case::base(rectangle(0, 0, 0, 5, 5))]
    #[case::asymmetric(rectangle(0, 0, 0, 274, 75))]
    #[case::ignore_index(rectangle(10, 10, 0, 10, 10))]
    fn new_draw_buffer(#[case] rect: Rectangle) -> Result<()> {
        let buf = DrawBuffer::new(rect.clone());
        let inner = buf.inner.lock().unwrap();
        assert_eq!(inner.buf.len(), rect.height());
        for row in &inner.buf {
            assert_eq!(row.len(), rect.width());
        }
        Ok(())
    }
}
