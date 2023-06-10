use std::io::Write;

use crossterm::{
    cursor,
    event::{self, Event as CrossTermEvent, KeyCode, KeyEvent},
    style,
    style::Color,
    style::Colors,
    terminal, ExecutableCommand, QueueableCommand,
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
    width: u16,
    height: u16,
    board: Board,
}

impl<T: Write> Tui48<T> {
    pub(crate) fn new(board: Board, w: Box<T>) -> Result<Self> {
        let (width, height) = terminal::size()?;
        Ok(Self {
            board,
            w,
            width,
            height,
        })
    }

    /// Run consumes the Tui48 instance and takes control of the terminal to begin gameplay.
    pub(crate) fn run(mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        self.w.execute(terminal::EnterAlternateScreen)?;

        self.initialize_terminal()?;
        self.draw_board()?;
        self.w.flush()?;

        loop {
            match wait_for_event()? {
                Event::UserInput(UserInput::Direction(d)) => self.shift(d)?,
                Event::UserInput(UserInput::Quit) => break,
            }
        }

        self.w.execute(terminal::LeaveAlternateScreen)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }
}

// High-level game board functions.
impl<T: Write> Tui48<T> {
    fn shift(&mut self, direction: Direction) -> Result<()> {
        if let Some(hint) = self.board.shift(direction) {}
        Ok(())
    }

    fn initialize_terminal(&mut self) -> Result<()> {
        self.w.queue(terminal::BeginSynchronizedUpdate)?;
        self.w.queue(cursor::SavePosition)?;
        self.w
            .queue(style::SetColors(Colors::new(Color::Grey, Color::Grey)))?;
        for x in 0..=self.width {
            for y in 0..=self.height {
                self.w.queue(cursor::MoveTo(x, y))?;
                self.w.queue(style::Print(" "))?;
            }
        }
        self.w.queue(style::ResetColor)?;
        self.w.queue(cursor::RestorePosition)?;
        self.w.queue(terminal::EndSynchronizedUpdate)?;
        Ok(())
    }

    fn draw_board(&mut self) -> Result<()> {
        self.w.queue(terminal::BeginSynchronizedUpdate)?;
        self.w.queue(cursor::SavePosition)?;
        self.w
            .queue(style::SetColors(Colors::new(Color::Black, Color::Grey)))?;
        for x in 0..=self.width {
            for y in 0..=self.height {
                self.w.queue(cursor::MoveTo(x, y))?;
                self.w.queue(style::Print('x'))?;
            }
        }
        self.w.queue(style::ResetColor)?;
        self.w.queue(cursor::RestorePosition)?;
        self.w.queue(terminal::EndSynchronizedUpdate)?;
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
