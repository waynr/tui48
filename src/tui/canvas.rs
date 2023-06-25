use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use super::drawbuffer::{DBTuxel, DrawBuffer};
use super::error::{Result, TuiError};
use super::geometry::{Bounds2D, Idx, Rectangle};
use super::tuxel::Tuxel;

struct CanvasInner {
    grid: Vec<Vec<Stack>>,
    rectangle: Rectangle,

    idx_receiver: Receiver<Idx>,
    idx_sender: Sender<Idx>,

    tuxel_receiver: Receiver<Tuxel>,
    tuxel_sender: Sender<Tuxel>,
}

impl CanvasInner {
    fn get_draw_buffer(&mut self, c: Canvas, r: Rectangle) -> Result<DrawBuffer> {
        self.reclaim();
        let modifiers = SharedModifiers::default();
        let mut dbuf = DrawBuffer::new(self.tuxel_sender.clone(), r.clone(), modifiers.clone(), c);
        for (y, row) in self
            .grid
            .iter_mut()
            .enumerate()
            .skip(r.y())
            .take(r.height())
        {
            for (x, cellstack) in row.iter_mut().enumerate().skip(r.x()).take(r.width()) {
                let canvas_idx = Idx(x, y, r.0 .2);
                let cell = cellstack.acquire(canvas_idx.z());
                let tuxel = match cell {
                    Cell::Tuxel(mut t) => {
                        t.shared_modifiers = Some(modifiers.clone());
                        t
                    }
                    _ => return Err(TuiError::CellAlreadyOwned),
                };
                let db_tuxel = dbuf.push(tuxel);
                cellstack.replace(canvas_idx.z(), Cell::DBTuxel(db_tuxel));
            }
        }
        Ok(dbuf)
    }

    fn get_layer(&mut self, c: Canvas, z: usize) -> Result<DrawBuffer> {
        self.get_draw_buffer(c, Rectangle(Idx(0, 0, z), self.rectangle.1.clone()))
    }

    fn draw_all(&mut self) -> Result<()> {
        for row in self.grid.iter_mut() {
            for stack in row.iter_mut() {
                self.idx_sender.send(stack.lock().idx.clone())?
            }
        }
        Ok(())
    }

    fn dimensions(&self) -> (usize, usize) {
        (self.rectangle.1 .0, self.rectangle.1 .1)
    }

    fn get_changed(&self) -> Vec<Stack> {
        let mut stacks = Vec::new();
        loop {
            match self.idx_receiver.try_recv() {
                Ok(idx) => stacks.push(self.grid[idx.1][idx.0].clone()),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    unreachable!();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
            }
        }
        stacks
    }

    fn reclaim(&mut self) {
        loop {
            match self.tuxel_receiver.try_recv() {
                Ok(tuxel) => {
                    let idx = tuxel.idx();
                    let _ = self.grid[idx.y()][idx.x()].replace(idx.z(), Cell::Tuxel(tuxel));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    unreachable!();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
            }
        }
    }

    fn acquire_cell(&mut self, idx: &Idx) -> Result<Cell> {
        Ok(self.grid
            .get_mut(idx.y())
            .ok_or(TuiError::OutOfBoundsY(idx.y()))?
            .get_mut(idx.x())
            .ok_or(TuiError::OutOfBoundsX(idx.x()))?
            .acquire(idx.z()))
    }

    fn replace_cell(&mut self, idx: &Idx, cell: Cell) -> Result<()> {
        Ok(self.grid
            .get_mut(idx.y())
            .ok_or(TuiError::OutOfBoundsY(idx.y()))?
            .get_mut(idx.x())
            .ok_or(TuiError::OutOfBoundsX(idx.x()))?
            .replace(idx.z(), cell))
    }

    fn swap_tuxels(&mut self, idx1: Idx, idx2: Idx) -> Result<()> {
        self.rectangle.contains_or_err(&idx1)?;
        self.rectangle.contains_or_err(&idx2)?;
        let mut c1 = self.acquire_cell(&idx1)?;
        let mut c2 = match self.acquire_cell(&idx2) {
            Err(e) => {
                // if we fail to get c2 we need to return c1
                self.replace_cell(&idx1, c1)?;
                return Err(e)
            }
            Ok(c) => c,
        };

        match &mut c1 {
            Cell::Empty => (),
            Cell::DBTuxel(ref mut dbt) => {
                dbt.set_canvas_idx(&idx2);
            },
            Cell::Tuxel(ref mut t) => {
                t.set_idx(&idx2);
            },
        }
        match &mut c2 {
            Cell::Empty => (),
            Cell::DBTuxel(ref mut dbt) => {
                dbt.set_canvas_idx(&idx1);
            },
            Cell::Tuxel(ref mut t) => {
                t.set_idx(&idx1);
            },
        }

        self.replace_cell(&idx1, c2)?;
        self.replace_cell(&idx2, c1)?;
        self.idx_sender.send(idx1)?;
        self.idx_sender.send(idx2)?;

        Ok(())
    }
}

/// A 2d grid of `Cell`s.
#[derive(Clone)]
pub(crate) struct Canvas {
    inner: Arc<Mutex<CanvasInner>>,
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
            inner: Arc::new(Mutex::new(CanvasInner {
                grid,
                rectangle,
                idx_sender,
                idx_receiver,
                tuxel_sender,
                tuxel_receiver,
            })),
        };
        s.draw_all().expect("enqueuing entire canvas rerender");

        s
    }

    fn lock(&self) -> MutexGuard<CanvasInner> {
        self.inner
            .lock()
            .expect("TODO: handle mutex lock errors more gracefully")
    }

    pub(crate) fn get_draw_buffer(&self, r: Rectangle) -> Result<DrawBuffer> {
        let c = self.clone();
        self.lock().get_draw_buffer(c, r)
    }

    pub(crate) fn get_layer(&self, z: usize) -> Result<DrawBuffer> {
        let c = self.clone();
        self.lock().get_layer(c, z)
    }

    pub(crate) fn draw_all(&mut self) -> Result<()> {
        self.lock().draw_all()
    }

    pub(crate) fn dimensions(&self) -> (usize, usize) {
        self.lock().dimensions()
    }

    pub(crate) fn get_changed(&self) -> Vec<Stack> {
        self.lock().get_changed()
    }

    pub(crate) fn swap_tuxels(&self, t1: Idx, t2: Idx) -> Result<()> {
        self.lock().swap_tuxels(t1, t2)
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
            Cell::Empty => Ok('\u{2622}'),
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
            Err(_) => Ok(()),
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

    fn acquire(&mut self, z: usize) -> Cell {
        self.lock().cells[z].take()
    }

    fn replace(&mut self, z: usize, cell: Cell) {
        let _ = self.lock().cells[z].replace(cell);
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
                    Err(_) => write!(f, "\u{2622}"),
                }
            }
            // show radioactive symbol if we can't find a character to show
            None => return Ok(()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
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

    fn is_dbtuxel(cell: &Cell) -> bool {
        match cell {
            Cell::DBTuxel(..) => true,
            _ => false,
        }
    }

    fn is_tuxel(cell: &Cell) -> bool {
        match cell {
            Cell::Tuxel(..) => true,
            _ => false,
        }
    }

    #[rstest]
    #[case::base((5, 5), rectangle(0, 0, 0, 5, 5))]
    #[case::realistic((274, 75), rectangle(0, 0, 0, 274, 75))]
    #[case::realistic_smaller_buffer((274, 75), rectangle(10, 10, 0, 10, 10))]
    fn validate_tuxel_reclaim(
        #[case] canvas_dims: (usize, usize),
        #[case] rect: Rectangle,
    ) -> Result<()> {
        let mut canvas = Canvas::new(canvas_dims.0, canvas_dims.1);
        let dbuf = canvas.get_draw_buffer(rect.clone())?;

        let mut idxs: Vec<Idx> = Vec::new();
        {
            let inner = dbuf.lock();
            for row in &inner.buf {
                for tuxel in row {
                    let idx = tuxel.idx();
                    let cell = &canvas.grid[idx.1][idx.0].lock().cells[idx.2];
                    assert!(is_dbtuxel(cell));
                    idxs.push(idx);
                }
            }
        }

        drop(dbuf);

        for idx in idxs.iter() {
            let cell = &canvas.grid[idx.1][idx.0].lock().cells[idx.2];
            assert!(is_dbtuxel(cell));
        }

        canvas.reclaim();

        for idx in idxs.iter() {
            let cell = &canvas.grid[idx.1][idx.0].lock().cells[idx.2];
            assert!(is_tuxel(cell));
        }

        Ok(())
    }
}
