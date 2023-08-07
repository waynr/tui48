use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};

use super::colors::Rgb;
use super::drawbuffer::{DBTuxel, DrawBuffer};
use super::error::{InnerError, Result, TuiError};
use super::geometry::{Bounds2D, Geometry, Idx, Rectangle};
use super::tuxel::Tuxel;

const CANVAS_DEPTH: usize = 8;

struct CanvasInner {
    grid: Vec<Vec<Stack>>,
    rectangle: Rectangle,

    idx_receiver: Receiver<Idx>,
    idx_sender: SyncSender<Idx>,

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
                    Cell::Empty => Tuxel::new(Idx(x, y, r.z()), self.idx_sender.clone()),
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
                    let _ = self.grid[idx.y()][idx.x()].replace(idx.z(), Cell::Empty);
                    self.idx_sender
                        .send(idx)
                        .expect("idx sender should have plenty of room for more idxes");
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

    fn swap_tuxels(&mut self, from_idx: Idx, to_idx: Idx) -> Result<()> {
        log::trace!("swapping {0} and {1}", from_idx, to_idx);
        self.rectangle.contains_or_err(Geometry::Idx(&from_idx))?;
        self.rectangle.contains_or_err(Geometry::Idx(&to_idx))?;
        let mut from_cell = self.acquire_cell(&from_idx)?;
        let mut to_cell = match self.acquire_cell(&to_idx) {
            Err(e) => {
                // if we fail to get to_cell we need to return from_cell
                self.replace_cell(&from_idx, from_cell)?;
                return Err(e);
            }
            Ok(c) => c,
        };

        match &mut from_cell {
            Cell::Empty => (),
            Cell::DBTuxel(ref mut dbt) => {
                match dbt.set_canvas_idx(&to_idx) {
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
        }
        match &mut to_cell {
            Cell::Empty => (),
            Cell::DBTuxel(ref mut dbt) => {
                match dbt.set_canvas_idx(&from_idx) {
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
        }

        self.replace_cell(&from_idx, to_cell)?;
        self.replace_cell(&to_idx, from_cell)?;
        self.idx_sender.send(from_idx)?;
        self.idx_sender.send(to_idx)?;

        Ok(())
    }

    fn swap_rectangles(&mut self, rect1: &Rectangle, rect2: &Rectangle) -> Result<()> {
        if rect1 == rect2 {
            return Ok(());
        } else if rect1.width() != rect2.width() || rect1.height() != rect2.height() {
            return Err(InnerError::RectangleDimensionsMustMatch.into());
        }

        let rect1_indices = rect1.clone().into_iter();
        let rect2_indices = rect2.clone().into_iter();
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
                    return true;
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
                continue;
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

        let (idx_sender, idx_receiver) = sync_channel(width*height*20);
        let (tuxel_sender, tuxel_receiver) = channel();
        let c = Self {
            inner: Arc::new(Mutex::new(CanvasInner {
                grid,
                rectangle,
                idx_sender,
                idx_receiver,
                tuxel_sender,
                tuxel_receiver,
            })),
        };

        c
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

    fn draw_all(&mut self) -> Result<()> {
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

    #[cfg(test)]
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
    DBTuxel(DBTuxel),
}

impl Cell {
    pub(crate) fn get_content(&self) -> Result<char> {
        match self {
            Cell::DBTuxel(b) => b.content(),
            Cell::Empty => Ok('\u{2622}'),
        }
    }

    pub(crate) fn active(&self) -> Result<bool> {
        match self {
            Cell::DBTuxel(b) => b.active(),
            Cell::Empty => Ok(false),
        }
    }

    pub(crate) fn colors(&self) -> (Option<Rgb>, Option<Rgb>) {
        match self {
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
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
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
        self.lock()
            .cells
            .iter()
            .nth(zdx)
            .map_or(false, |c| match c {
                Cell::Empty => false,
                Cell::DBTuxel(_) => true,
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
            Cell::DBTuxel(_) => "D",
        }
    }
}

impl Stack {
    pub(crate) fn coordinates(&self) -> (usize, usize) {
        let idx = self.lock().idx.clone();
        (idx.x(), idx.y())
    }

    pub(crate) fn colors(&self) -> (Option<Rgb>, Option<Rgb>) {
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
            Some(' ')
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum Modifier {
    SetForegroundColor(u8, u8, u8),
    SetBackgroundColor(u8, u8, u8),
    SetBGLightness(f32),
    SetFGLightness(f32),
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
            _ => (fgcolor, bgcolor),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use rstest::*;

    use super::super::geometry;
    use super::*;

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
        let canvas = Canvas::new(dims.0, dims.1);
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
        let canvas = Canvas::new(canvas_dims.0, canvas_dims.1);
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

    fn is_empty(cell: &Cell) -> bool {
        match cell {
            Cell::Empty => true,
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
        let canvas = Canvas::new(canvas_dims.0, canvas_dims.1);
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
            assert!(is_empty(cell));
        }

        canvas.lock().reclaim();

        for idx in idxs.iter() {
            let inner = canvas.lock();
            let cell = &inner.grid[idx.1][idx.0].lock().cells[idx.2];
            assert!(is_empty(cell));
        }

        Ok(())
    }

    #[rstest]
    #[case::base((50, 50), rectangle(0, 0, 0, 2, 2), (1, geometry::Direction::Down))]
    fn validate_drawbuffer_translation_cleanup(
        #[case] canvas_dims: (usize, usize),
        #[case] initial_db_rect: Rectangle,
        #[case] mv: (usize, geometry::Direction),
    ) -> Result<()> {
        let canvas = Canvas::new(canvas_dims.0, canvas_dims.1);

        // verify blank canvas doesn't have any changed indices
        let mut canvas_changed_idxs: BTreeSet<(usize, usize)> = BTreeSet::new();
        for stack in canvas.get_changed() {
            canvas_changed_idxs.insert(stack.coordinates());
        }
        assert_eq!(
            canvas_changed_idxs.iter().count(),
            0,
            "expected no changed indices, found:\n {:?}",
            canvas_changed_idxs,
        );

        let mut dbuf = canvas.get_draw_buffer(initial_db_rect.clone())?;
        let (dbuf_width, dbuf_height) = dbuf.rectangle().dimensions();

        // verify creation of drawbuffer itself doesn't result in changed indices
        let mut canvas_changed_idxs: BTreeSet<(usize, usize)> = BTreeSet::new();
        for stack in canvas.get_changed() {
            canvas_changed_idxs.insert(stack.coordinates());
        }
        assert_eq!(
            canvas_changed_idxs.iter().count(),
            0,
            "expected no changed indices, found:\n {:?}",
            canvas_changed_idxs,
        );

        // fill drawbuffer so its tuxels are considered active
        dbuf.fill('.')?;

        {
            // validate the number of changed indices from drawing
            let mut canvas_changed_idxs: BTreeSet<(usize, usize)> = BTreeSet::new();
            for stack in canvas.get_changed() {
                canvas_changed_idxs.insert(stack.coordinates());
            }
            assert_eq!(
                canvas_changed_idxs.iter().count(),
                dbuf_width * dbuf_height,
                "expected {} changed indices, found:\n {:?}",
                dbuf_width * dbuf_height,
                canvas_changed_idxs,
            );
        }

        // verify there are no changed indicies without doing anything to the drawbuffer after
        // running get_changed() above
        let mut canvas_changed_idxs: BTreeSet<(usize, usize)> = BTreeSet::new();
        for stack in canvas.get_changed() {
            canvas_changed_idxs.insert(stack.coordinates());
        }
        assert_eq!(
            canvas_changed_idxs.iter().count(),
            0,
            "expected no changed indices, found:\n {:?}",
            canvas_changed_idxs,
        );

        //  calculate the set of changed IDXs based on all the canvase indices touched by the
        //  drawbuffer
        let mut post_translation_rect = initial_db_rect.clone();
        post_translation_rect.translate(mv.0, &mv.1)?;
        let initial_rect_indices = initial_db_rect.into_iter();
        let post_translation_rect_indices = post_translation_rect.into_iter();

        let mut expected_changed_idxs: BTreeSet<(usize, usize)> = BTreeSet::new();
        assert_eq!(
            0,
            expected_changed_idxs.iter().count(),
            "expected {} changed indices, found:\n {:?}",
            0,
            canvas_changed_idxs,
        );
        for idx in initial_rect_indices {
            expected_changed_idxs.insert((idx.0, idx.1));
        }
        assert_eq!(
            (dbuf_width) * dbuf_height,
            expected_changed_idxs.iter().count(),
            "expected {} changed indices, found:\n {:?}",
            (dbuf_width) * dbuf_height,
            canvas_changed_idxs,
        );

        for idx in post_translation_rect_indices {
            expected_changed_idxs.insert((idx.0, idx.1));
        }

        //  obtain set of changed IDXs from the canvas
        dbuf.translate(mv.1)?;

        let mut canvas_changed_idxs: BTreeSet<(usize, usize)> = BTreeSet::new();
        for stack in canvas.get_changed() {
            canvas_changed_idxs.insert(stack.coordinates());
        }
        assert_eq!(
            canvas_changed_idxs.iter().count(),
            (dbuf_width + 1) * dbuf_height,
            "expected {} changed indices, found:\n {:?}",
            (dbuf_width + 1) * dbuf_height,
            canvas_changed_idxs,
        );

        assert_eq!(
            expected_changed_idxs.iter().count(),
            (dbuf_width + 1) * dbuf_height,
            "expected {} changed indices, found:\n {:?}",
            (dbuf_width + 1) * dbuf_height,
            canvas_changed_idxs,
        );

        let canvas_changed_idx_count = canvas_changed_idxs.iter().count();
        let expected_changed_idx_count = expected_changed_idxs.iter().count();
        assert_eq!(
            expected_changed_idx_count, canvas_changed_idx_count,
            "\nexpected:\n {:?}\nactual:\n {:?}",
            expected_changed_idxs, canvas_changed_idxs
        );

        // use set logic to verify changed canvas IDXs
        let only_in_canvas = canvas_changed_idxs.difference(&expected_changed_idxs);
        let only_in_expected = expected_changed_idxs.difference(&canvas_changed_idxs);

        assert!(
            only_in_expected.clone().count() == 0,
            "missing changed indices in the canvas {:?}",
            &only_in_expected
        );
        assert!(
            only_in_canvas.clone().count() == 0,
            "found unexpected changed indices in the canvas {:?}",
            &only_in_canvas
        );

        Ok(())
    }
}
