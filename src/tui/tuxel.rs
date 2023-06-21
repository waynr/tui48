use std::sync::{Arc, Mutex, MutexGuard};

use super::canvas::{Modifier, SharedModifiers};
use super::geometry::Idx;

#[derive(Default)]
pub(crate) struct Tuxel {
    active: bool,
    content: char,
    idx: Idx,
    modifiers: Vec<Modifier>,
    pub(crate) shared_modifiers: Option<SharedModifiers>,
}

impl Tuxel {
    pub(crate) fn new(idx: Idx) -> Self {
        Tuxel {
            // use radioactive symbol to indicate user hasn't set a value for this Tuxel.
            //content: '\u{2622}',
            //content: '\u{2566}',
            active: false,
            content: '-',
            idx,
            modifiers: Vec::new(),
            shared_modifiers: None,
        }
    }

    pub(crate) fn set_content(&mut self, c: char) {
        self.active = true;
        self.content = c;
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.idx.0, self.idx.1)
    }

    pub(crate) fn modifiers(&self) -> Vec<Modifier> {
        let mut modifiers = match &self.shared_modifiers {
            Some(ms) => ms.lock().clone(),
            None => Vec::new(),
        };
        modifiers.append(&mut self.modifiers.clone());
        modifiers.to_vec()
    }

    pub(crate) fn clear(&mut self) {
        self.content = ' ';
        self.modifiers.clear();
        self.shared_modifiers = None;
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
}

impl std::fmt::Display for Tuxel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content())?;
        Ok(())
    }
}
