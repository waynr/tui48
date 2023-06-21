use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use super::drawbuffer::{DBTuxel, DrawBuffer};
use super::error::{Result, TuiError};
use super::geometry::{Bounds2D, Idx, Rectangle};
use super::tuxel::Tuxel;

/// A 2d grid of `Cell`s.
pub(crate) struct Canvas {
    grid: Vec<Vec<Stack>>,
    rectangle: Rectangle,

    idx_receiver: Receiver<Idx>,
    idx_sender: Sender<Idx>,

    tuxel_receiver: Receiver<Tuxel>,
    tuxel_sender: Sender<Tuxel>,
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
        let (idx_sender, idx_receiver) = channel();
        let (tuxel_sender, tuxel_receiver) = channel();
        let mut s = Self {
            grid,
            rectangle,
            idx_sender,
            idx_receiver,
            tuxel_sender,
            tuxel_receiver,
        };
        s.draw_all().expect("enqueuing entire canvas rerender");

        s
    }

    pub(crate) fn get_draw_buffer(&mut self, r: Rectangle) -> Result<DrawBuffer> {
        let modifiers = SharedModifiers::default();
        let mut dbuf = DrawBuffer::new(self.tuxel_sender.clone(), r.clone(), modifiers.clone());
        for (buf_y, (y, row)) in self
            .grid
            .iter_mut()
            .enumerate()
            .skip(r.y())
            .take(r.height())
            .enumerate()
        {
            for (x, cellstack) in row.iter_mut().enumerate().skip(r.x()).take(r.width()) {
                let canvas_idx = Idx(x, y, r.0 .2);
                let cell = cellstack.acquire(canvas_idx.clone(), modifiers.clone())?;
                let tuxel = match cell {
                    Cell::Tuxel(t) => t,
                    _ => return Err(TuiError::CellAlreadyOwned),
                };
                let db_tuxel = dbuf.push(tuxel);
                cellstack.replace(canvas_idx, Cell::DBTuxel(db_tuxel));
            }
        }
        Ok(dbuf)
    }

    pub(crate) fn get_layer(&mut self, z: usize) -> Result<DrawBuffer> {
        self.get_draw_buffer(Rectangle(Idx(0, 0, z), self.rectangle.1.clone()))
    }

    pub(crate) fn draw_all(&mut self) -> Result<()> {
        for row in self.grid.iter_mut() {
            for stack in row.iter_mut() {
                self.idx_sender.send(stack.lock().idx.clone())?
            }
        }
        Ok(())
    }

    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.rectangle.1 .0, self.rectangle.1 .1)
    }

    fn get_stack(&mut self, idx: Idx) -> Result<Stack> {
        Ok(self.grid[idx.1][idx.0].clone())
    }

    pub(crate) fn reclaim(&mut self) {
        loop {
            match self.tuxel_receiver.try_recv() {
                Ok(tuxel) => {
                    let idx = tuxel.idx();
                    let _ = self.grid[idx.1][idx.0].replace(idx, Cell::Tuxel(tuxel));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    unreachable!();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
            }
        }
    }
}

impl Iterator for &Canvas {
    type Item = Stack;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.idx_receiver.try_recv() {
                Ok(idx) => return Some(self.grid[idx.1][idx.0].clone()),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    unreachable!();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            }
        }
    }
}

#[derive(Default)]
pub(crate) enum Cell {
    #[default]
    Empty,
    Tuxel(Tuxel),
    DBTuxel(DBTuxel),
}

impl Cell {
    pub(crate) fn get_content(&self) -> Result<char> {
        match self {
            Cell::Tuxel(t) => Ok(t.content()),
            Cell::DBTuxel(b) => b.content(),
            Cell::Empty => Ok('x'),
        }
    }

    pub(crate) fn active(&self) -> Result<bool> {
        match self {
            Cell::Tuxel(t) => Ok(t.active()),
            Cell::DBTuxel(b) => b.active(),
            Cell::Empty => Ok(false),
        }
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        match self {
            Cell::Tuxel(t) => t.coordinates(),
            Cell::DBTuxel(d) => d.coordinates(),
            Cell::Empty => (0, 0),
        }
    }

    pub(crate) fn modifiers(&self) -> Result<Vec<Modifier>> {
        match self {
            Cell::Tuxel(t) => Ok(t.modifiers()),
            Cell::DBTuxel(d) => d.modifiers(),
            Cell::Empty => Ok(Vec::new()),
        }
    }

    fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    fn replace(&mut self, other: Self) -> Self {
        std::mem::replace(self, other)
    }
}

impl std::fmt::Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.get_content() {
            Ok(v) => write!(f, "{}", v),
            Err(e) => Ok(()),
        }
    }
}

/// A stack of `Cells`. Enables z-ordering of elements with occlusion and update detection. Tuxels
/// are wrapped in a Arc<Mutex<_>> to allow them to be referenced by the higher level Widget
/// abstraction at the same time.
#[derive(Default)]
struct StackInner {
    cells: [Cell; 8],
    idx: Idx,
}

#[derive(Clone, Default)]
pub(crate) struct Stack {
    inner: Arc<Mutex<StackInner>>,
}

impl Stack {
    fn new(x: usize, y: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StackInner {
                idx: Idx(x, y, 0),
                cells: [
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 0))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 1))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 2))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 3))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 4))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 5))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 6))),
                    Cell::Tuxel(Tuxel::new(Idx(x, y, 7))),
                ],
            })),
        }
    }

    fn acquire(&mut self, idx: Idx, shared_modifiers: SharedModifiers) -> Result<Cell> {
        Ok(self.lock().cells[idx.2].take())
    }

    fn replace(&mut self, idx: Idx, cell: Cell) {
        let _ = self.lock().cells[idx.2].replace(cell);
    }

    fn top(&self) -> Option<usize> {
        self.lock()
            .cells
            // low-index elements of a stack are below high-index elements. we want to find the
            // first active tuxel on top of the stack so we iterate over elements in reverse
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, c)| match c.active() {
                Ok(b) if b == true => Some(idx),
                _ => None,
            })
    }

    fn lock(&self) -> MutexGuard<StackInner> {
        self.inner
            .lock()
            .expect("TODO: handle mutex lock errors more gracefully")
    }
}

impl Stack {
    pub(crate) fn modifiers(&self) -> Result<Vec<Modifier>> {
        if let Some(idx) = self.top() {
            self.lock().cells[idx].modifiers()
        } else {
            Ok(Vec::new())
        }
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        if let Some(idx) = self.top() {
            self.lock().cells[idx].coordinates()
        } else {
            (0, 0)
        }
    }
}

impl std::fmt::Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.top() {
            Some(idx) => {
                match self
                    .lock()
                    .cells
                    .get(idx)
                    .expect("if Stack.top() returns an index that element must exist")
                    .get_content()
                {
                    Ok(c) => {
                        write!(f, "{}", c)
                    }
                    // show radioactive symbol if we can't find a character to show
                    Err(_) => write!(f, "x"),
                }
            }
            // show radioactive symbol if we can't find a character to show
            None => write!(f, "x"),
        }
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
    fn get_layer_validate_draw_buffer_size(#[case] dims: (usize, usize)) -> Result<()> {
        let mut canvas = Canvas::new(dims.0, dims.1);
        let dbuf = canvas.get_layer(0)?;
        let inner = dbuf.lock();
        assert_eq!(inner.buf.len(), dims.1);
        for row in &inner.buf {
            assert_eq!(row.len(), dims.0);
        }
        Ok(())
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

        let inner = buffer.lock();
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
}
