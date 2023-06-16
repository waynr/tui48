use std::io::Write;

use anyhow::Context;
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
        self.w.execute(cursor::Show);
        self.w
            .execute(terminal::LeaveAlternateScreen)
            .expect("leaving alternate screen");
        terminal::disable_raw_mode().expect("disabling raw mode");
    }
}

impl<T: Write> Renderer for Crossterm<T> {
    fn render(&mut self, c: &Canvas) -> Result<()> {
        self.w
            .queue(terminal::BeginSynchronizedUpdate)
            .with_context(|| "queue synchronized update")?;
        self.w
            .queue(cursor::SavePosition)
            .with_context(|| "queue save cursor position")?;
        for result in c {
            let tuxel = result.with_context(|| "getting tuxel")?;
            let tuxel = tuxel.lock();
            if tuxel.active() {
                for command in tuxel.modifiers().iter() {
                    self.queue(command)
                        .with_context(|| "queue tuxel modifier")?;
                }
                let (x, y) = tuxel.coordinates();
                self.w
                    .queue(cursor::MoveTo(x as u16, y as u16))
                    .with_context(|| "queue moving cursor")?;
                self.w
                    .queue(style::Print(format!("{}", &tuxel)))
                    .with_context(|| "queue printing tuxel text")?;
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

pub(crate) fn size() -> Result<(u16, u16)> {
    Ok(terminal::size().with_context(|| "get terminal size")?)
}

/// Block until the next Crossterm event.
pub(crate) fn next_event() -> Result<Event> {
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
