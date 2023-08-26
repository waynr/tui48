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

impl std::fmt::Display for TextBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.lock().fmt(f)
    }
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
        let mut y_offset = 0;
        let mut x_offset = 0;

        if inner.border {
            rect = rect.shrink_by(1, 1);
            y_offset += 1;
            x_offset += 1;
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
            (VAlignment::Top, _) => (0usize + y_offset, 0usize),
            (_, Ordering::Equal) => (0usize + y_offset, 0usize),
            (VAlignment::Middle, Ordering::Less) => {
                let difference = rect.height() - bufs.len();
                let y_index = difference / 2 + difference % 2;
                (y_index + y_offset, 0)
            }
            (VAlignment::Middle, Ordering::Greater) => {
                let difference = bufs.len() - rect.height();
                let buf_skip = difference / 2;
                (0 + y_offset, buf_skip)
            }
            (VAlignment::Bottom, Ordering::Less) => {
                let y_index = rect.height() - bufs.len();
                (y_index + y_offset, 0)
            }
            (VAlignment::Bottom, Ordering::Greater) => {
                let buf_skip = bufs.len() - rect.height();
                (0 + y_offset, buf_skip)
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
                HAlignment::Center => width_diff / 2 + width_diff % 2,
                HAlignment::Right => width_diff,
            } + x_offset;

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
    use rstest::*;

    use super::super::geometry::{Bounds2D, Idx, Rectangle};
    use super::*;

    fn from_strs(ss: Vec<&str>) -> Vec<Vec<char>> {
        ss.into_iter()
            .map(|s| s.chars().collect::<Vec<char>>())
            .collect()
    }

    fn add_borders(cc: &mut Vec<Vec<char>>) {
        let box_corner = boxy::Char::upper_left(boxy::Weight::Doubled);
        let box_horizontal = boxy::Char::horizontal(boxy::Weight::Doubled);
        let box_vertical = boxy::Char::vertical(boxy::Weight::Doubled);

        // get current buffer width, assuming all rows are the same width
        let width = cc.first().unwrap().len();

        // draw side borders first before adding new top and bottom row containing borders
        for row in cc.iter_mut() {
            row.insert(0, box_vertical.clone().into());
            row.push(box_vertical.clone().into());
        }

        let mut top: Vec<char> = [Into::<char>::into(box_horizontal)].into_iter().cycle().take(width).collect();
        let mut bottom: Vec<char> = top.clone();

        top.insert(0, box_corner.clone().into());
        top.push(box_corner.rotate_cw(1).clone().into());
        bottom.insert(0, box_corner.clone().rotate_ccw(1).into());
        bottom.push(box_corner.rotate_cw(2).clone().into());

        cc.insert(0, top);
        cc.push(bottom);
    }

    fn fo(halign: HAlignment, valign: VAlignment) -> Option<FormatOptions> {
        Some(FormatOptions { halign, valign })
    }

    enum Border {
        On,
        Off,
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
    #[case::wrapping_default(None, "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "meowmeowme",
        "    ow    ",
        "          ",
    ]))]
    #[case::wrapping_center_middle(fo(HAlignment::Center, VAlignment::Middle), "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "meowmeowme",
        "    ow    ",
        "          ",
    ]))]
    #[case::wrapping_center_top(fo(HAlignment::Center, VAlignment::Top), "meowmeowmeow", from_strs(vec![
        "meowmeowme",
        "    ow    ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::wrapping_center_bottom(fo(HAlignment::Center, VAlignment::Bottom), "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "meowmeowme",
        "    ow    ",
    ]))]
    #[case::wrapping_left_middle(fo(HAlignment::Left, VAlignment::Middle), "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "meowmeowme",
        "ow        ",
        "          ",
    ]))]
    #[case::wrapping_left_top(fo(HAlignment::Left, VAlignment::Top), "meowmeowmeow", from_strs(vec![
        "meowmeowme",
        "ow        ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::wrapping_left_bottom(fo(HAlignment::Left, VAlignment::Bottom), "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "meowmeowme",
        "ow        ",
    ]))]
    #[case::wrapping_right_middle(fo(HAlignment::Right, VAlignment::Middle), "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "meowmeowme",
        "        ow",
        "          ",
    ]))]
    #[case::wrapping_right_top(fo(HAlignment::Right, VAlignment::Top), "meowmeowmeow", from_strs(vec![
        "meowmeowme",
        "        ow",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::wrapping_right_bottom(fo(HAlignment::Right, VAlignment::Bottom), "meowmeowmeow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "meowmeowme",
        "        ow",
    ]))]
    #[case::wrapping_multiword_default(None, "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        " meow meow",
        "   meow   ",
        "          ",
    ]))]
    #[case::wrapping_multiword_center_middle(fo(HAlignment::Center, VAlignment::Middle),
                                             "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        " meow meow",
        "   meow   ",
        "          ",
    ]))]
    #[case::wrapping_multiword_center_top(fo(HAlignment::Center, VAlignment::Top),
                                             "meow meow meow", from_strs(vec![
        " meow meow",
        "   meow   ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::wrapping_multiword_center_bottom(fo(HAlignment::Center, VAlignment::Bottom),
                                             "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        " meow meow",
        "   meow   ",
    ]))]
    #[case::wrapping_multiword_left_middle(fo(HAlignment::Left, VAlignment::Middle),
                                             "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        "meow meow ",
        "meow      ",
        "          ",
    ]))]
    #[case::wrapping_multiword_left_top(fo(HAlignment::Left, VAlignment::Top),
                                             "meow meow meow", from_strs(vec![
        "meow meow ",
        "meow      ",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::wrapping_multiword_left_bottom(fo(HAlignment::Left, VAlignment::Bottom),
                                             "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        "meow meow ",
        "meow      ",
    ]))]
    #[case::wrapping_multiword_right_middle(fo(HAlignment::Right, VAlignment::Middle),
                                             "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        " meow meow",
        "      meow",
        "          ",
    ]))]
    #[case::wrapping_multiword_right_top(fo(HAlignment::Right, VAlignment::Top),
                                             "meow meow meow", from_strs(vec![
        " meow meow",
        "      meow",
        "          ",
        "          ",
        "          ",
    ]))]
    #[case::wrapping_multiword_right_bottom(fo(HAlignment::Right, VAlignment::Bottom),
                                             "meow meow meow", from_strs(vec![
        "          ",
        "          ",
        "          ",
        " meow meow",
        "      meow",
    ]))]
    fn validate_formatting_no_border(
        #[case] fo: Option<FormatOptions>,
        #[case] text: &str,
        #[case] mut expected: Vec<Vec<char>>,
        #[values(Border::On, Border::Off)] border: Border,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let bounds = match border {
            Border::On => Bounds2D(12, 7),
            Border::Off => Bounds2D(10, 5),
        };
        let rect = Rectangle(Idx(0, 0, 0), bounds);
        let canvas = Canvas::new(20, 20);
        let mut tbuf = canvas.get_text_buffer(rect.clone())?;

        if let Some(fo) = fo {
            tbuf.format(fo);
        }

        match border {
            Border::On => {
                add_borders(&mut expected);
                tbuf.draw_border()?;
            },
            _ => (),
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
                assert_eq!(
                    actual,
                    expected,
                    "expected char '{}' at ({}, {}), got '{}'\nactual drawbuffer:\n{}",
                    expected,
                    idx.x(),
                    idx.y(),
                    actual,
                    inner,
                );
            }
        }

        Ok(())
    }
}
