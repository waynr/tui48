use std::cmp::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use textwrap::wrap;

use super::canvas::{Canvas, Modifier};
use super::colors::Rgb;
use super::drawbuffer::{DrawBufferInner, DrawBufferOwner};
use super::error::{InnerError, Result};
use super::geometry::{Position, Rectangle};
use super::tuxel::Tuxel;

#[derive(Clone, Default, PartialEq)]
pub(crate) enum HAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Default, PartialEq)]
pub(crate) enum VAlignment {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[derive(Clone, Default, PartialEq)]
pub(crate) struct FormatOptions {
    halign: HAlignment,
    valign: VAlignment,
}

pub(crate) struct CharBuf {
    text: String,
    fgcolor: Option<Rgb>,
    bgcolor: Option<Rgb>,
}

impl CharBuf {
    fn wrap(&self, width: usize) -> Vec<CharBuf> {
        wrap(&self.text, width)
            .into_iter()
            .map(|s| CharBuf {
                text: s.to_string(),
                fgcolor: self.fgcolor.clone(),
                bgcolor: self.bgcolor.clone(),
            })
            .collect()
    }

    #[inline]
    fn len(&self) -> usize {
        self.text.len()
    }
}

/// A line-oriented buffer that makes writing structured/formatted text to DrawBuffers somewhat
/// easier.
pub(crate) struct TextBuffer {
    bufs: Vec<CharBuf>,
    inner: Arc<Mutex<DrawBufferInner>>,
    format: FormatOptions,
    sender: Sender<Tuxel>,
}

impl TextBuffer {
    pub(crate) fn new(sender: Sender<Tuxel>, rectangle: Rectangle, canvas: Canvas) -> Self {
        let mut buf: Vec<_> = Vec::with_capacity(rectangle.height());
        for _ in 0..rectangle.height() {
            let row: Vec<Tuxel> = Vec::with_capacity(rectangle.width());
            buf.push(row);
        }
        Self {
            bufs: Vec::new(),
            inner: Arc::new(Mutex::new(DrawBufferInner {
                rectangle,
                border: false,
                buf,
                modifiers: Vec::new(),
                canvas,
            })),
            format: FormatOptions::default(),
            sender,
        }
    }

    pub fn format(&mut self, format: FormatOptions) {
        if self.format == format {
            return;
        }
        self.format = format
    }

    pub fn write(&mut self, s: &str, fgcolor: Option<Rgb>, bgcolor: Option<Rgb>) {
        self.bufs.push(CharBuf {
            text: s.to_string(),
            fgcolor,
            bgcolor,
        })
    }

    pub fn flush(&mut self) -> Result<()> {
        let mut inner = self.lock();
        let mut rect = inner.rectangle.clone();
        if inner.border {
            rect = rect.shrink_by(1, 1);
        }

        if rect.width() == 0 || rect.height() == 0 {
            return Ok(());
        }

        let bufs = self
            .bufs
            .iter()
            .map(|cb| cb.wrap(rect.width()))
            .flatten()
            .collect::<Vec<CharBuf>>();

        let (mut y_index, buf_skip) = match (&self.format.valign, bufs.len().cmp(&rect.height())) {
            (VAlignment::Top, _) => (0usize, 0usize),
            (_, Ordering::Equal) => (0usize, 0usize),
            (VAlignment::Middle, Ordering::Less) => {
                let difference = rect.height() - bufs.len();
                let y_index = difference / 2;
                (y_index, 0)
            }
            (VAlignment::Middle, Ordering::Greater) => {
                let difference = bufs.len() - rect.height();
                let buf_skip = difference / 2;
                (0, buf_skip)
            }
            (VAlignment::Bottom, Ordering::Less) => {
                let y_index = rect.height() - bufs.len();
                (y_index, 0)
            }
            (VAlignment::Bottom, Ordering::Greater) => {
                let buf_skip = bufs.len() - rect.height();
                (0, buf_skip)
            }
        };

        let bufs_iter = bufs.iter().skip(buf_skip);

        for charbuf in bufs_iter {
            let buflen = charbuf.len();

            if y_index > rect.height() {
                // can't write beyond the bottom of the rectangle
                break;
            }

            let width_diff = if buflen > rect.width() {
                // we shouldn't reach this point because we wrapped on the rectangle width earlier.
                return Err(InnerError::OutOfBoundsX(buflen).into());
            } else {
                rect.width() - buflen
            };

            let x_index = match &self.format.halign {
                HAlignment::Left => 0,
                HAlignment::Center => width_diff / 2,
                HAlignment::Right => width_diff,
            };

            for (offset, c) in charbuf.text.chars().enumerate() {
                let pos = Position::Coordinates(x_index + offset, y_index);
                let tuxel = inner.get_tuxel_mut(pos)?;
                tuxel.set_content(c);
                if let Some(c) = &charbuf.bgcolor {
                    tuxel.set_bgcolor(c.clone());
                }
                if let Some(c) = &charbuf.fgcolor {
                    tuxel.set_fgcolor(c.clone());
                }
            }

            y_index += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
impl TextBuffer {
    pub(crate) fn set_sender(&mut self, sender: Sender<Tuxel>) {
        self.sender = sender
    }
}

impl DrawBufferOwner for TextBuffer {
    fn lock<'a>(&'a self) -> MutexGuard<'a, DrawBufferInner> {
        match self.inner.as_ref().lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
    }

    fn inner(&self) -> Arc<Mutex<DrawBufferInner>> {
        self.inner.clone()
    }
}

impl Drop for TextBuffer {
    fn drop(&mut self) {
        let mut inner = self.lock();
        for row in inner.buf.iter_mut() {
            while let Some(mut tuxel) = row.pop() {
                tuxel.clear();
                // can't do anything about send errors here -- we rely on the channel having the
                // necessary capacity and the Canvas outliving all DrawBuffers
                let _ = self.sender.send(tuxel);
            }
        }
        let _ = inner.canvas.reclaim();
    }
}

#[cfg(test)]
mod test {
    use std::sync::mpsc::channel;

    use rstest::*;

    use super::super::geometry::{Bounds2D, Idx, Rectangle};
    use super::*;

    fn from_strs(ss: Vec<&str>) -> Vec<Vec<char>> {
        ss.into_iter()
            .map(|s| s.chars().collect::<Vec<char>>())
            .collect()
    }

    fn fo(halign: HAlignment, valign: VAlignment) -> Option<FormatOptions> {
        Some(FormatOptions { halign, valign })
    }

    #[rstest]
    #[case::default(None, "meow", from_strs(vec![
        "          ",
        "          ",
        "   meow   ",
        "          ",
        "          ",
    ]))]
    #[case::center_middle(fo(HAlignment::Center, VAlignment::Middle), "meow", from_strs(vec![
        "          ",
        "          ",
        "   meow   ",
        "          ",
        "          ",
    ]))]
    #[case::center_top(fo(HAlignment::Center, VAlignment::Top), "meow", from_strs(vec![
        "   meow   ",
        "          ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::center_bottom(fo(HAlignment::Center, VAlignment::Bottom), "meow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "          ",
        "   meow   ",
    ]))]
    #[case::left_middle(fo(HAlignment::Left, VAlignment::Middle), "meow", from_strs(vec![
        "          ",
        "          ",
        "meow      ",
        "          ",
        "          ",
    ]))]
    #[case::left_top(fo(HAlignment::Left, VAlignment::Top), "meow", from_strs(vec![
        "meow      ",
        "          ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::left_bottom(fo(HAlignment::Left, VAlignment::Bottom), "meow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "          ",
        "meow      ",
    ]))]
    #[case::right_middle(fo(HAlignment::Right, VAlignment::Middle), "meow", from_strs(vec![
        "          ",
        "          ",
        "      meow",
        "          ",
        "          ",
    ]))]
    #[case::right_top(fo(HAlignment::Right, VAlignment::Top), "meow", from_strs(vec![
        "      meow",
        "          ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::right_bottom(fo(HAlignment::Right, VAlignment::Bottom), "meow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "          ",
        "      meow",
    ]))]
    fn validate_formatting_no_border(
        #[case] fo: Option<FormatOptions>,
        #[case] text: &str,
        #[case] expected: Vec<Vec<char>>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let rect = Rectangle(Idx(0, 0, 0), Bounds2D(10, 5));
        let canvas = Canvas::new(20, 20);
        let mut tbuf = canvas.get_text_buffer(rect.clone())?;

        if let Some(fo) = fo {
            tbuf.format(fo);
        }

        tbuf.fill(' ')?;
        tbuf.write(text, None, None);
        tbuf.flush()?;

        let rect = (&tbuf as &dyn DrawBufferOwner).rectangle();
        let indices = rect.into_iter();
        {
            let inner = tbuf.lock();
            for idx in indices {
                let t = inner.get_tuxel(Position::Idx(idx.clone()))?;
                let row = expected
                    .get(idx.y())
                    .ok_or(InnerError::OutOfBoundsY(idx.y()))?;
                let expected = row
                    .get(idx.x())
                    .ok_or(InnerError::OutOfBoundsX(idx.x()))?
                    .clone();
                let actual = t.content();
                assert_eq!(actual, expected);
            }
        }

        Ok(())
    }
}
