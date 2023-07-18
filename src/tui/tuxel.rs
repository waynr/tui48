use std::sync::mpsc::SyncSender;

use super::colors::Rgb;
use super::geometry::Idx;

pub(crate) struct Tuxel {
    active: bool,
    content: char,
    idx: Idx,
    idx_sender: SyncSender<Idx>,
    fgcolor: Option<Rgb>,
    bgcolor: Option<Rgb>,
}

impl Tuxel {
    pub(crate) fn new(idx: Idx, idx_sender: SyncSender<Idx>) -> Self {
        Tuxel {
            active: false,
            content: '-',
            fgcolor: None,
            bgcolor: None,
            idx,
            idx_sender,
        }
    }

    pub(crate) fn set_content(&mut self, c: char) {
        self.active = true;
        self.content = c;
        self.idx_sender
            .send(self.idx.clone())
            .expect("idx sender has a big buffer, it shouldn't fail");
    }

    pub(crate) fn clear(&mut self) {
        self.active = false;
        self.content = ' ';
        self.idx_sender
            .send(self.idx.clone())
            .expect("idx sender has a big buffer, it shouldn't fail");
    }

    pub(crate) fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn content(&self) -> char {
        self.content
    }

    pub(crate) fn idx(&self) -> Idx {
        self.idx.clone()
    }

    pub(crate) fn set_idx(&mut self, idx: &Idx) {
        self.idx = idx.clone();
        self.idx_sender
            .send(self.idx.clone())
            .expect("idx sender has a big buffer, it shouldn't fail");
    }

    pub(crate) fn colors(&self) -> (Option<Rgb>, Option<Rgb>) {
        (self.fgcolor.clone(), self.bgcolor.clone())
    }
}

impl std::fmt::Display for Tuxel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content())
    }
}
