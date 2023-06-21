use std::io::Write;

use anyhow::Context;
use crossterm::{
    cursor,
    event::{self, Event as CrossTermEvent, KeyCode, KeyEvent},
    style,
    style::Color,
    terminal, ExecutableCommand, QueueableCommand,
};

use super::canvas::{Canvas, Modifier};
use super::error::Result;
use super::events::{Direction, Event, EventSource, UserInput};
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
        self.w.execute(cursor::Show).expect("showing cursor again");
        self.w
            .execute(terminal::LeaveAlternateScreen)
            .expect("leaving alternate screen");
        terminal::disable_raw_mode().expect("disabling raw mode");
    }
}

impl<T: Write> Renderer for Crossterm<T> {
    fn clear(&mut self, c: &Canvas) -> Result<()> {
        let (width, height) = c.dimensions();
        self.w
            .queue(terminal::BeginSynchronizedUpdate)
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
            .queue(terminal::EndSynchronizedUpdate)
            .with_context(|| "queue end synchronized update")?;
        self.w.flush().with_context(|| "flush writer")?;
        Ok(())
    }

    fn render(&mut self, c: &Canvas) -> Result<()> {
        self.w
            .queue(terminal::BeginSynchronizedUpdate)
            .with_context(|| "queue synchronized update")?;
        self.w
            .queue(cursor::SavePosition)
            .with_context(|| "queue save cursor position")?;
        for result in c {
            let cell = result.with_context(|| "getting tuxel")?;
            if cell.active()? {
                for command in cell.modifiers()?.iter() {
                    self.queue(command)
                        .with_context(|| "queue tuxel modifier")?;
                }
                let (x, y) = cell.coordinates();
                self.w
                    .queue(cursor::MoveTo(x as u16, y as u16))
                    .with_context(|| "queue moving cursor")?;
                self.w
                    .queue(style::Print(format!("{}", &cell)))
                    .with_context(|| "queue printing cell text")?;
                self.w
                    .queue(style::ResetColor)
                    .with_context(|| "queue color reset")?;
                self.w
                    .queue(style::SetAttribute(style::Attribute::Reset))
                    .with_context(|| "queue attribute reset")?;
            };
        }
        self.w
            .queue(cursor::RestorePosition)
            .with_context(|| "queue restore position")?;
        self.w
            .queue(terminal::EndSynchronizedUpdate)
            .with_context(|| "queue end synchronized update")?;
        self.w.flush().with_context(|| "flush writer")?;
        Ok(())
    }

    fn size_hint(&self) -> Result<(u16, u16)> {
        size()
    }
}

impl<T: Write> Crossterm<T> {
    fn queue(&mut self, m: &Modifier) -> Result<()> {
        match m {
            Modifier::BackgroundColor(r, g, b) => self
                .w
                .queue(style::SetBackgroundColor(Color::Rgb {
                    r: *r,
                    g: *g,
                    b: *b,
                }))
                .with_context(|| "queue setting background color")?,
            Modifier::ForegroundColor(r, g, b) => self
                .w
                .queue(style::SetForegroundColor(Color::Rgb {
                    r: *r,
                    g: *g,
                    b: *b,
                }))
                .with_context(|| "queue setting foreground color")?,
            Modifier::Bold => self
                .w
                .queue(style::SetAttribute(style::Attribute::Bold))
                .with_context(|| "queue setting bold attribute")?,
        };
        Ok(())
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
