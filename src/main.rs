use std::cell::RefCell;
use std::rc::Rc;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
};

use rand::{thread_rng, Rng};
use rand::rngs::ThreadRng;

/// The Result type for mastermind.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Board {
    rng: ThreadRng,
    basket: ValueBasket,
    slots: [[Slot;4] ;4]
}

impl Board {
    fn new(mut rng: ThreadRng) -> Self {
        let (xdx2, ydx2) = (0, 0);
        let (xdx1, ydx1) = (rng.gen_range(0..3), rng.gen_range(0..3));
        loop {
            let (xdx2, ydx2) = (rng.gen_range(0..3), rng.gen_range(0..3));
            if (xdx1, ydx1) == (xdx2, ydx2) {
                continue;
            }
            break;
        }
        let mut b = Self {
            rng,
            basket: ValueBasket::default(),
            slots: [
                [Slot::default(), Slot::default(), Slot::default(), Slot::default()],
                [Slot::default(), Slot::default(), Slot::default(), Slot::default()],
                [Slot::default(), Slot::default(), Slot::default(), Slot::default()],
                [Slot::default(), Slot::default(), Slot::default(), Slot::default()],
            ],
        };
        b.slots[ydx1][xdx1].set(b.basket.get());
        b.slots[ydx2][xdx2].set(b.basket.get());
        b
    }
}

#[derive(Default)]
struct Slot {
    value: Option<Rc<RefCell<Value>>>,
}

impl Slot {
    fn set(&mut self, value: Rc<RefCell<Value>>) {
        self.value = Some(value)
    }

    fn unset(&mut self) {
        self.value = None
    }
}

struct Value(u16);

impl Default for Value {
    fn default() -> Self {
        Value(2)
    }
}

#[derive(Default)]
struct ValueBasket {
    values: [Rc<RefCell<Value>>; 25],
}

impl ValueBasket {
    fn get(&self) -> Rc<RefCell<Value>> {
        let v = self.values
            .iter()
            .find(|item| Rc::<_>::strong_count(item) == 1)
            .expect("something is wrong if we can't find a value here!")
            .clone();
        if v.borrow().0 != 2 {
            let mut mutv = v.borrow_mut();
            mutv.0 = 2;
        }
        v
    }
}

fn main() -> Result<()> {
    let mut board = Board::default();
    let value_buf = ValueBasket::default();

    board.slots[0][0].set(value_buf.get());

    terminal::enable_raw_mode()?;
    while let Event::Key(KeyEvent { code, .. }) = event::read()? {
        match code {
            KeyCode::Enter => {
                break;
            }
            KeyCode::Char(c) => {
                break;
            }
            _ => {}
        }
    }
    terminal::disable_raw_mode()?;

    return Ok(());
}
