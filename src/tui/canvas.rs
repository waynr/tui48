use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

use super::colors::Rgb;
use super::drawbuffer::{DBTuxel, DrawBuffer};
use super::error::{InnerError, Result, TuiError};
use super::geometry::{Bounds2D, Geometry, Idx, Indices, Rectangle};
use super::tuxel::Tuxel;

const CANVAS_DEPTH: usize = 8;

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
        self.rectangle.contains_or_err(Geometry::Rectangle(&r))?;
        let mut dbuf = DrawBuffer::new(self.tuxel_sender.clone(), r.clone(), c);
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
                    Cell::Tuxel(t) => t,
                    _ => return Err(InnerError::CellAlreadyOwned.into()),
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

    fn bounds(&self) -> Bounds2D {
        self.rectangle.1.clone()
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
        Ok(self
            .grid
            .get_mut(idx.y())
            .ok_or(InnerError::OutOfBoundsY(idx.y()))?
            .get_mut(idx.x())
            .ok_or(InnerError::OutOfBoundsX(idx.x()))?
            .acquire(idx.z()))
    }

    fn replace_cell(&mut self, idx: &Idx, cell: Cell) -> Result<()> {
        Ok(self
            .grid
            .get_mut(idx.y())
            .ok_or(InnerError::OutOfBoundsY(idx.y()))?
            .get_mut(idx.x())
            .ok_or(InnerError::OutOfBoundsX(idx.x()))?
            .replace(idx.z(), cell))
    }

    fn swap_tuxels(&mut self, idx1: Idx, idx2: Idx) -> Result<()> {
        log::trace!("swapping {0} and {1}", idx1, idx2);
        self.rectangle.contains_or_err(Geometry::Idx(&idx1))?;
        self.rectangle.contains_or_err(Geometry::Idx(&idx2))?;
        let mut c1 = self.acquire_cell(&idx1)?;
        let mut c2 = match self.acquire_cell(&idx2) {
            Err(e) => {
                // if we fail to get c2 we need to return c1
                self.replace_cell(&idx1, c1)?;
                return Err(e);
            }
            Ok(c) => c,
        };

        match &mut c1 {
            Cell::Empty => (),
            Cell::DBTuxel(ref mut dbt) => {
                match dbt.set_canvas_idx(&idx2) {
                    Ok(_) => (),
                    // if we hit retry limit, assume that this change is ultimately being driven by
                    // the DrawBuffer whose tuxels we are attempting to update and that the
                    // DrawBuffer code will take responsibility for updating it as necessary
                    Err(TuiError {
                        inner: InnerError::ExceedRetryLimitForLockingDrawBuffer(_),
                        ..
                    }) => (),
                    Err(e) => return Err(e),
                }
            }
            Cell::Tuxel(ref mut t) => {
                t.set_idx(&idx2);
            }
        }
        match &mut c2 {
            Cell::Empty => (),
            Cell::DBTuxel(ref mut dbt) => {
                match dbt.set_canvas_idx(&idx2) {
                    Ok(_) => (),
                    // if we hit retry limit, assume that this change is ultimately being driven by
                    // the DrawBuffer whose tuxels we are attempting to update and that the
                    // DrawBuffer code will take responsibility for updating it as necessary
                    Err(TuiError {
                        inner: InnerError::ExceedRetryLimitForLockingDrawBuffer(_),
                        ..
                    }) => (),
                    Err(e) => return Err(e),
                }
            }
            Cell::Tuxel(ref mut t) => {
                t.set_idx(&idx1);
            }
        }

        self.replace_cell(&idx1, c2)?;
        self.replace_cell(&idx2, c1)?;
        self.idx_sender.send(idx1)?;
        self.idx_sender.send(idx2)?;

        Ok(())
    }

    fn swap_rectangles(&mut self, rect1: &Rectangle, rect2: &Rectangle) -> Result<()> {
        if rect1 == rect2 {
            return Ok(());
        } else if rect1.width() != rect2.width() || rect1.height() != rect2.height() {
            return Err(InnerError::RectangleDimensionsMustMatch.into());
        }

        let rect1_indices: Indices = rect1.clone().into();
        let rect2_indices: Indices = rect2.clone().into();
        log::trace!("swapping {0} and {1}", rect1, rect2);
        for (idx1, idx2) in rect1_indices.zip(rect2_indices) {
            self.swap_tuxels(idx1, idx2)?;
        }
        self.reclaim();
        Ok(())
    }

    fn layer_occupied(&self, zdx: usize) -> bool {
        for row in self.grid.iter() {
            for stack in row.iter() {
                if stack.layer_occupied(zdx) {
                    return true
                }
            }
        }
        false
    }
}

impl std::fmt::Display for CanvasInner {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for i in 0..CANVAS_DEPTH {
            if !self.layer_occupied(i) {
                continue
            }
            write!(f, "canvas layer {}:\n", i)?;
            for row in self.grid.iter() {
                for stack in row.iter() {
                    write!(f, "{}", stack.display_cell_type(i))?;
                }
                write!(f, "\n")?;
            }
            write!(f, "\n")?;
            write!(f, "\n")?;
        }
        Ok(())
    }
}

/// A 2d grid of `Cell`s.
#[derive(Clone)]
pub(crate) struct Canvas {
    inner: Arc<Mutex<CanvasInner>>,
}

impl std::fmt::Display for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.lock())
    }
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

    pub(crate) fn bounds(&self) -> Bounds2D {
        self.lock().bounds()
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

    pub(crate) fn swap_rectangles(&self, r1: &Rectangle, r2: &Rectangle) -> Result<()> {
        self.lock().swap_rectangles(r1, r2)
    }

    pub(crate) fn layer_occupied(&self, zdx: usize) -> bool {
        self.lock().layer_occupied(zdx)
    }

    pub(crate) fn reclaim(&mut self) -> Result<()> {
        self.lock().reclaim();
        Ok(())
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

    pub(crate) fn colors(&self) -> (Option<Rgb>, Option<Rgb>) {
        match self {
            Cell::Tuxel(t) => t.colors(),
            Cell::DBTuxel(d) => d.colors(),
            Cell::Empty => (None, None),
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
    cells: [Cell; CANVAS_DEPTH],
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

    fn layer_occupied(&self, zdx: usize) -> bool {
        self.lock().cells.iter().nth(zdx).map_or(false, |c| match c {
            Cell::Empty => false,
            Cell::DBTuxel(_) => true,
            Cell::Tuxel(_) => false,
        })
    }

    fn lock(&self) -> MutexGuard<StackInner> {
        self.inner
            .lock()
            .expect("TODO: handle mutex lock errors more gracefully")
    }

    fn display_cell_type(&self, zdx: usize) -> &str {
        match &self.lock().cells[zdx] {
            Cell::Empty => "E",
            Cell::Tuxel(t) => "T",
            Cell::DBTuxel(t) => "D",
        }
    }
}

impl Stack {
    pub(crate) fn coordinates(&self) -> (usize, usize) {
        if let Some(idx) = self.top() {
            self.lock().cells[idx].coordinates()
        } else {
            (0, 0)
        }
    }

    pub(crate) fn colors(&self) -> (Option<Rgb>, Option<Rgb>) {
        let fg = Rgb::default();
        let bg = Rgb::default();
        if let Some(idx) = self.top() {
            self.lock()
                .cells
                .get(idx)
                .expect("if Stack.top() returns an index that element must exist")
                .colors()
        } else {
            (None, None)
        }
    }

    pub(crate) fn content(&self) -> Option<char> {
        if let Some(idx) = self.top() {
            self.lock()
                .cells
                .get(idx)
                .expect("if Stack.top() returns an index that element must exist")
                .get_content()
                .ok()
        } else {
            None
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum Modifier {
    SetForegroundColor(u8, u8, u8),
    SetBackgroundColor(u8, u8, u8),
    SetBGLightness(f32),
    SetFGLightness(f32),
    AdjustLightnessBG(f32),
    AdjustLightnessFG(f32),
}

impl Modifier {
    pub(crate) fn apply(
        &self,
        (fgcolor, bgcolor): (Option<Rgb>, Option<Rgb>),
    ) -> (Option<Rgb>, Option<Rgb>) {
        match (fgcolor.clone(), bgcolor.clone(), self) {
            (_, bgcolor, Modifier::SetForegroundColor(r, g, b)) => {
                (Some(Rgb::new(*r, *g, *b)), bgcolor)
            }
            (fgcolor, _, Modifier::SetBackgroundColor(r, g, b)) => {
                (fgcolor, Some(Rgb::new(*r, *g, *b)))
            }
            (Some(fgcolor), bgcolor, Modifier::SetFGLightness(l)) => {
                (Some(fgcolor.set_lightness(*l)), bgcolor)
            }
            (fgcolor, Some(bgcolor), Modifier::SetBGLightness(l)) => {
                (fgcolor, Some(bgcolor.set_lightness(*l)))
            }
            (Some(fgcolor), bgcolor, Modifier::AdjustLightnessFG(l)) => {
                (Some(fgcolor.adjust_lightness(*l)), bgcolor)
            }
            (fgcolor, Some(bgcolor), Modifier::AdjustLightnessBG(l)) => {
                (fgcolor, Some(bgcolor.adjust_lightness(*l)))
            }
            _ => (fgcolor, bgcolor),
        }
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
        assert_eq!(canvas.lock().grid.len(), dims.1);
        for row in &canvas.lock().grid {
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
                    let inner = canvas.lock();
                    let cell = &inner.grid[idx.1][idx.0].lock().cells[idx.2];
                    assert!(is_dbtuxel(cell));
                    idxs.push(idx);
                }
            }
        }

        drop(dbuf);

        for idx in idxs.iter() {
            let inner = canvas.lock();
            let cell = &inner.grid[idx.1][idx.0].lock().cells[idx.2];
            assert!(is_tuxel(cell));
        }

        canvas.lock().reclaim();

        for idx in idxs.iter() {
            let inner = canvas.lock();
            let cell = &inner.grid[idx.1][idx.0].lock().cells[idx.2];
            assert!(is_tuxel(cell));
        }

        Ok(())
    }
}
