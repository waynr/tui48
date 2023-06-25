use std::io::stdout;

use rand::thread_rng;

mod error;
mod engine;
mod tui;
mod tui48;

use engine::board::Board;
use error::Result;
use tui48::{init, Tui48};
use tui::crossterm::{Crossterm, CrosstermEvents};

fn main() -> Result<()> {
    let rng = thread_rng();
    let board = Board::new(rng);
    let w = stdout().lock();
    let renderer = Crossterm::new(Box::new(w))?;
    let event_source = CrosstermEvents::default();
    let tui48 = Tui48::new(board, renderer, event_source)?;

    init()?;

    Ok(tui48.run()?)
}
