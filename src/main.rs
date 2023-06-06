use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
};
use rand::thread_rng;

mod board;
mod error;
mod round;

use board::{Board, Direction};
use error::Result;

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
            _ => {
                let _hint = board.shift(Direction::Left);
            }
        }
    }
    terminal::disable_raw_mode()?;

    return Ok(());
}
