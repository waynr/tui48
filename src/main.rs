use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
};
use rand::thread_rng;

mod board;
mod round;
mod error;

use error::Result;
use board::Board;

fn main() -> Result<()> {
    let rng = thread_rng();
    let mut board = Board::new(rng);

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
