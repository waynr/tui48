use super::geometry::Idx;
use super::colors::Rgb;

#[derive(Default)]
pub(crate) struct Tuxel {
    active: bool,
    content: char,
    idx: Idx,
    fgcolor: Option<Rgb>,
    bgcolor: Option<Rgb>,
}

impl Tuxel {
    pub(crate) fn new(idx: Idx) -> Self {
        Tuxel {
            // use radioactive symbol to indicate user hasn't set a value for this Tuxel.
            //content: '\u{2622}',
            //content: '\u{2566}',
            active: false,
            content: '-',
            fgcolor: None,
            bgcolor: None,
            idx,
        }
    }

    pub(crate) fn set_content(&mut self, c: char) {
        self.active = true;
        self.content = c;
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.idx.0, self.idx.1)
    }

    pub(crate) fn clear(&mut self) {
        self.active = false;
        self.content = ' ';
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
        self.idx = idx.clone()
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
