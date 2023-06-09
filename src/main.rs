use std::io::stdout;

use rand::thread_rng;

mod board;
mod error;
mod round;
mod tui;

use board::Board;
use error::Result;
use tui::Tui48;

fn main() -> Result<()> {
    let rng = thread_rng();
    let board = Board::new(rng);
    let w = stdout().lock();
    let tui48 = Tui48::new(board, Box::new(w))?;

    tui48.run()
}
