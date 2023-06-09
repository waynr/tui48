use std::io::Write;

use crossterm::{
    event::{self, Event as CrossTermEvent, KeyCode, KeyEvent},
    terminal,
    terminal::{
        EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand,
};

use crate::board::{Board, Direction};
use crate::error::Result;

enum Event {
    UserInput(UserInput),
}

enum UserInput {
    Direction(Direction),
    Quit,
}

pub(crate) struct Tui48<T: Write> {
    w: Box<T>,
    board: Board,
}

impl<T: Write> Tui48<T> {
    pub(crate) fn new(board: Board, w: Box<T>) -> Self {
        Self { board, w }
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    pub(crate) fn run(mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        self.w.execute(EnterAlternateScreen)?;

        loop {
            match wait_for_event()? {
                Event::UserInput(UserInput::Direction(d)) => self.shift(d)?,
                Event::UserInput(UserInput::Quit) => break,
            }
        }

        self.w.execute(LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}

impl<T: Write> Tui48<T> {
    fn shift(&mut self, direction: Direction) -> Result<()> {
        if let Some(hint) = self.board.shift(direction) {}
        Ok(())
    }
}

fn wait_for_event() -> Result<Event> {
    loop {
        match event::read()? {
            CrossTermEvent::Key(ke) => match handle_key_event(ke) {
                Some(ke) => return Ok(Event::UserInput(ke)),
                None => continue,
            },
            _ => continue,
        };
    }
}

fn handle_key_event(ke: KeyEvent) -> Option<UserInput> {
    match ke {
        KeyEvent { code, .. } => match code {
            KeyCode::Left | KeyCode::Char('h') => Some(UserInput::Direction(Direction::Left)),
            KeyCode::Right | KeyCode::Char('l') => Some(UserInput::Direction(Direction::Right)),
            KeyCode::Up | KeyCode::Char('k') => Some(UserInput::Direction(Direction::Up)),
            KeyCode::Down | KeyCode::Char('j') => Some(UserInput::Direction(Direction::Down)),
            KeyCode::Char('q') => Some(UserInput::Quit),
            _ => None,
        },
        _ => None,
    }
}
