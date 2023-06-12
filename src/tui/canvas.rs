use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};

use crate::error::{Error, Result};

/// Idx encapsulates the x, y, and z coordinates of a Tuxel-based shape.
#[derive(Clone, Default)]
pub(crate) struct Idx(usize, usize, usize);

#[derive(Clone, Default)]
pub(crate) struct Bounds2D(usize, usize);

#[derive(Clone, Default)]
pub(crate) struct Rectangle(Idx, Bounds2D);

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
        let mut buf = DrawBuffer::new(r.clone());
        for (y, row) in self.grid.iter_mut().enumerate().skip(r.0.1).take(r.1.1) {
            for (x, cellstack) in row.iter_mut().enumerate().skip(r.0.0).take(r.1.0) {
                let idx = Idx(x, y, r.0.2);
                let tuxel = cellstack.acquire(idx.clone(), Some(buf.clone()))?;
                buf.insert(&idx, tuxel)?;
            }
        }
        Ok(buf)
    }

    pub(crate) fn get_layer(&mut self, z: usize) -> Result<DrawBuffer> {
        self.get_draw_buffer(Rectangle(Idx(0,0,z), self.rectangle.1.clone()))
    }

    pub(crate) fn draw_all(&mut self) -> Result<()> {
        for (y, row) in self.grid.iter().enumerate() {
            for (x, stack) in row.iter().enumerate() {
                self.sender.send(stack.clone())?
            }
        }
        Ok(())
    }

    fn translate_tuxels(&mut self, ts: Vec<Arc<RwLock<Tuxel>>>) -> Result<()> {
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
/// are wrapped in a Arc<RwLock<_>> to allow them to be referenced by the higher level Widget
/// abstraction at the same time.
#[derive(Default)]
struct StackInner {
    cells: [Tuxel; 8],
    top_index: usize,
}

#[derive(Clone, Default)]
struct Stack {
    inner: Arc<RwLock<StackInner>>,
}

impl Stack {
    fn acquire(&mut self, idx: Idx, buf: Option<DrawBuffer>) -> Result<Tuxel> {
        let clone = self.inner.clone();
        let mut inner = clone.write().expect("lock unexpectedly poisoned");
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
        let readable = cloned.read().expect("lock unexpectedly poisoned");

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
    inner: Option<Arc<RwLock<TuxelInner>>>,
}

impl std::fmt::Display for Tuxel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = match &self.inner {
            Some(inner) => inner,
            None => return Ok(()),
        };
        write!(
            f,
            "{}",
            inner
                .read()
                .expect("TODO: handle thread panicking better than this")
                .content
        )?;
        Ok(())
    }
}

impl Tuxel {
    fn new(idx: Idx, buf: Option<DrawBuffer>) -> Self {
        Tuxel {
            inner: Some(Arc::new(RwLock::new(TuxelInner {
                // use radioactive symbol to indicate user hasn't set a value for this Tuxel.
                //content: '\u{2622}',
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

    pub(crate) fn before(&self) -> Vec<Modifier> {
        let inner = match &self.inner {
            Some(inner) => inner,
            None => return Vec::new(),
        };
        let mut parent_modifiers: Vec<Modifier> = if let Some(parent) = &inner
            .read()
            .expect("TODO: handle thread panicking better than this")
            .partof
        {
            parent
                .inner
                .read()
                .expect("TOOD: handle thread panicking better than this")
                .before_modifiers
                .iter()
                .map(|m| m.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut modifiers: Vec<Modifier> = inner
            .read()
            .expect("TODO: handle thread panicking better than this")
            .before_modifiers
            .clone();
        parent_modifiers.append(&mut modifiers);
        parent_modifiers
    }

    pub(crate) fn after(&self) -> Vec<Modifier> {
        let inner = match &self.inner {
            Some(inner) => inner,
            None => return Vec::new(),
        };
        let mut parent_modifiers: Vec<Modifier> = if let Some(parent) = &inner
            .read()
            .expect("TODO: handle thread panicking better than this")
            .partof
        {
            parent
                .inner
                .read()
                .expect("TODO: handle thread panicking better than this")
                .after_modifiers
                .iter()
                .map(|m| m.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut modifiers: Vec<Modifier> = inner
            .read()
            .expect("TODO: handle thread panicking better than this")
            .after_modifiers
            .clone();
        parent_modifiers.append(&mut modifiers);
        parent_modifiers
    }
}

#[derive(Default)]
struct DrawBufferInner {
    rectangle: Rectangle,
    buf: Vec<Vec<Tuxel>>,
    before_modifiers: Vec<Modifier>,
    after_modifiers: Vec<Modifier>,
}

#[derive(Clone, Default)]
pub(crate) struct DrawBuffer {
    inner: Arc<RwLock<DrawBufferInner>>,
}

impl DrawBuffer {
    fn new(rectangle: Rectangle) -> Self {
        let Bounds2D(width, height) = rectangle.1;
        let mut buf: Vec<_> = Vec::with_capacity(height);
        for _ in 0..height {
            let mut row: Vec<Tuxel> = Vec::with_capacity(width);
            for _ in 0..width {
                row.push(Tuxel::default())
            }
            buf.push(row);
        }
        Self {
            inner: Arc::new(RwLock::new(DrawBufferInner {
                rectangle,
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
            .write()
            .expect("TODO: handle thread panicking better than this");
        let current = inner
            .buf
            .get_mut(idx.1)
            .ok_or(Error::OutOfBounds)?
            .get_mut(idx.0)
            .ok_or(Error::OutOfBounds)?;
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
            .write()
            .expect("TODO: handle thread panicking better than this");
        inner.before_modifiers.push(modifier)
    }

    pub(crate) fn modify_after(&mut self, modifier: Modifier) {
        let mut inner = self
            .inner
            .write()
            .expect("TODO: handle thread panicking better than this");
        inner.after_modifiers.push(modifier)
    }
}

#[derive(Clone)]
pub(crate) enum Modifier {
    ForegroundColor(u8, u8, u8),
    BackgroundColor(u8, u8, u8),
    Bold,
}
