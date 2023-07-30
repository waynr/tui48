use std::io::Write;

use anyhow::Context;
use crossterm::{
    cursor,
    event::{self, Event as CrossTermEvent, KeyCode, KeyEvent},
    style,
    terminal, ExecutableCommand, QueueableCommand,
};

use super::canvas::Canvas;
use super::error::Result;
use super::events::{Event, EventSource, UserInput};
use super::geometry::Direction;
use super::renderer::Renderer;

pub(crate) struct Crossterm<T: Write> {
    w: Box<T>,
}

impl<T: Write> Crossterm<T> {
    pub(crate) fn new(mut w: Box<T>) -> Result<Self> {
        terminal::enable_raw_mode().with_context(|| "queue enabling raw mode")?;
        w.execute(terminal::EnterAlternateScreen)
            .with_context(|| "queue entering alternate screen")?;
        w.execute(cursor::Hide)
            .with_context(|| "queue hiding cursor")?;
        Ok(Self { w })
    }
}

impl<T: Write> Drop for Crossterm<T> {
    fn drop(&mut self) {
        self.recover();
    }
}

impl<T: Write> Renderer for Crossterm<T> {
    fn clear(&mut self, c: &Canvas) -> Result<()> {
        let (width, height) = c.dimensions();
        self.w
            .execute(terminal::BeginSynchronizedUpdate)
            .with_context(|| "queue synchronized update")?;
        self.w
            .queue(cursor::SavePosition)
            .with_context(|| "queue save cursor position")?;
        for x in 0..width {
            for y in 0..height {
                self.w
                    .queue(cursor::MoveTo(x as u16, y as u16))
                    .with_context(|| "queue moving cursor")?;
                self.w
                    .queue(style::Print(" "))
                    .with_context(|| "queue printing tuxel text")?;
            }
        }
        self.w
            .queue(cursor::RestorePosition)
            .with_context(|| "queue restore position")?;
        self.w
            .execute(terminal::EndSynchronizedUpdate)
            .with_context(|| "queue end synchronized update")?;
        self.w.flush().with_context(|| "flush writer")?;
        Ok(())
    }

    fn render(&mut self, c: &Canvas) -> Result<()> {
        self.w
            .execute(terminal::BeginSynchronizedUpdate)
            .with_context(|| "execute synchronized update")?;
        self.w
            .execute(cursor::SavePosition)
            .with_context(|| "execute save cursor position")?;
        for stack in c.get_changed() {
            let (fgcolor, bgcolor) = stack.colors();
            let output = match stack.content() {
                Some(c) => c,
                None => continue,
            };
            let (x, y) = stack.coordinates();
            self.w
                .execute(cursor::MoveTo(x as u16, y as u16))
                .with_context(|| "execute moving cursor")?;
            if let Some(bg) = bgcolor {
                self.w.execute(style::SetBackgroundColor(bg.into()))?;
            }
            if let Some(fg) = fgcolor {
                self.w.execute(style::SetForegroundColor(fg.into()))?;
            }
            self.w
                .execute(style::Print(output))
                .with_context(|| "execute printing cell text")?;
            self.w
                .execute(style::ResetColor)
                .with_context(|| "execute color reset")?;
            self.w
                .execute(style::SetAttribute(style::Attribute::Reset))
                .with_context(|| "execute attribute reset")?;
        }
        self.w
            .execute(cursor::RestorePosition)
            .with_context(|| "execute restore position")?;
        self.w
            .execute(terminal::EndSynchronizedUpdate)
            .with_context(|| "execute end synchronized update")?;
        Ok(())
    }

    fn size_hint(&self) -> Result<(u16, u16)> {
        size()
    }

    fn recover(&mut self) {
        self.w.execute(cursor::Show).expect("showing cursor again");
        self.w
            .execute(terminal::LeaveAlternateScreen)
            .expect("leaving alternate screen");
        terminal::disable_raw_mode().expect("disabling raw mode");
    }
}

#[derive(Default)]
pub(crate) struct CrosstermEvents {}

impl EventSource for CrosstermEvents {
    fn next_event(&self) -> Result<Event> {
        loop {
            match event::read().with_context(|| "read crossterm events")? {
                CrossTermEvent::Resize(_, _) => return Ok(Event::Resize),
                CrossTermEvent::Key(ke) => match handle_key_event(ke) {
                    Some(ke) => return Ok(Event::UserInput(ke)),
                    None => continue,
                },
                _ => continue,
            };
        }
    }
}

fn size() -> Result<(u16, u16)> {
    Ok(terminal::size().with_context(|| "get terminal size")?)
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
    }
}
