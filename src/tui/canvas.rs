use std::cell::RefCell;
use std::io::Write;
use std::ops::Add;
use std::rc::Rc;

/// A 2d grid of `Cell`s.
struct Canvas {
    grid: Vec<Vec<CellStack>>,
}

/// A `Canvas` grid element. Can either be empty or a `Tuxel`.
struct Cell {
    tuxel: Option<Tuxel(Rc<RefCell<Tuxel>>)>,
    stack_notifier: Rc<RefCell<bool>>,
}

/// A stack of `Cells`. Enables z-ordering of elements with occlusion and update
/// detection. Tuxels are wrapped in a Rc<RefCell<_>> to allow them to be referenced by the higher
/// level Widget abstraction at the same time.
struct CellStack {
    cells: [Cell; 16],
    top_index: usize,
    updated: Rc<RefCell<bool>>,
}

enum Modifier {
    ForegroundColor(u8, u8, u8),
    BackgroundColor(u8, u8, u8),
    Bold,
}

struct Tuxel {
    content: char,
    modifiers: Vec<Modifier>,
}

struct DrawBuffer {
    width: u16,
    height: u16,
    buf: Vec<Rc<RefCell<Tuxel>>>,
    modifiers: Vec<Modifier>,
}
