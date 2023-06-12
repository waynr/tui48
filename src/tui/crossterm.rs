use std::io::Write;

use crossterm::{
    cursor,
    event::{self, Event as CrossTermEvent, KeyCode, KeyEvent},
    style,
    style::Color,
    terminal, ExecutableCommand, QueueableCommand,
};

use crate::board::Direction;
use crate::error::Result;
use crate::tui::canvas::Canvas;
use crate::tui::{Event, Modifier, Renderer, UserInput};

pub(crate) struct Crossterm<T: Write> {
    w: Box<T>,
}

impl<T: Write> Crossterm<T> {
    pub(crate) fn new(mut w: Box<T>) -> Result<Self> {
        terminal::enable_raw_mode()?;
        w.execute(terminal::EnterAlternateScreen)?;
        Ok(Self { w })
    }
}

impl<T: Write> Drop for Crossterm<T> {
    fn drop(&mut self) {
        self.w
            .execute(terminal::LeaveAlternateScreen)
            .expect("leaving alternate screen");
        terminal::disable_raw_mode().expect("disabling raw mode");
    }
}

impl<T: Write> Renderer for Crossterm<T> {
    fn render(&mut self, c: &Canvas) -> Result<()> {
        self.w.queue(terminal::BeginSynchronizedUpdate)?;
        self.w.queue(cursor::SavePosition)?;
        self.w.queue(style::ResetColor)?;
        for result in c {
            if let Some(tuxel) = result?.lock() {
                for command in tuxel.before().iter() {
                    self.queue(command)?;
                }
                let (x, y) = tuxel.coordinates();
                self.w.queue(cursor::MoveTo(x as u16, y as u16))?;
                self.w.queue(style::Print(format!("{}", &tuxel)))?;
                for command in tuxel.after().iter() {
                    self.queue(command)?;
                }
            };
        }
        self.w.queue(cursor::RestorePosition)?;
        self.w.queue(terminal::EndSynchronizedUpdate)?;
        self.w.flush()?;
        Ok(())
    }
}

impl<T: Write> Crossterm<T> {
    fn queue(&mut self, m: &Modifier) -> Result<()> {
        match m {
            Modifier::BackgroundColor(r, g, b) => {
                self.w.queue(style::SetBackgroundColor(Color::Rgb {
                    r: *r,
                    g: *g,
                    b: *b,
                }))?
            }
            Modifier::ForegroundColor(r, g, b) => {
                self.w.queue(style::SetForegroundColor(Color::Rgb {
                    r: *r,
                    g: *g,
                    b: *b,
                }))?
            }
            Modifier::Bold => self.w.queue(style::SetAttribute(style::Attribute::Bold))?,
        };
        Ok(())
    }
}

pub(crate) fn size() -> Result<(u16, u16)> {
    Ok(terminal::size()?)
}

/// Block until the next Crossterm event.
pub(crate) fn next_event() -> Result<Event> {
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
