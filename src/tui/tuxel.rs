use std::sync::{Arc, Mutex, MutexGuard};

use super::Idx;
use super::canvas::{Modifier, SharedModifiers};

#[derive(Clone, Default)]
struct TuxelInner {
    active: bool,
    content: char,
    idx: Idx,
    modifiers: Vec<Modifier>,
    shared_modifiers: SharedModifiers,
}

impl TuxelInner {
    pub(crate) fn set_content(&mut self, c: char) {
        self.active = true;
        self.content = c;
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        (self.idx.0, self.idx.1)
    }

    pub(crate) fn modifiers(&self) -> Vec<Modifier> {
        let parent_modifiers = &mut self.shared_modifiers.lock();
        let mut modifiers: Vec<Modifier> = self.modifiers.clone();
        parent_modifiers.append(&mut modifiers);
        parent_modifiers.to_vec()
    }

    fn clear(&mut self) {
        self.content = ' ';
        self.modifiers.clear();
    }

    pub(crate) fn active(&self) -> bool {
        self.active
    }
}

#[derive(Clone, Default)]
pub(crate) struct Tuxel {
    inner: Arc<Mutex<TuxelInner>>,
}

impl Tuxel {
    pub(crate) fn new(idx: Idx) -> Self {
        Tuxel {
            inner: Arc::new(Mutex::new(TuxelInner {
                // use radioactive symbol to indicate user hasn't set a value for this Tuxel.
                //content: '\u{2622}',
                //content: '\u{2566}',
                active: false,
                content: 'x',
                idx,
                modifiers: Vec::new(),
                shared_modifiers: SharedModifiers::default(),
            })),
        }
    }

    pub(crate) fn content(&self) -> char {
        self.lock().content
    }

    pub(crate) fn set_content(&mut self, c: char) {
        self.lock().set_content(c)
    }

    pub(crate) fn coordinates(&self) -> (usize, usize) {
        self.lock().coordinates()
    }

    pub(crate) fn modifiers(&self) -> Vec<Modifier> {
        self.lock().modifiers()
    }

    pub(crate) fn clear(&mut self) {
        self.lock().clear()
    }

    pub(crate) fn active(&self) -> bool {
        self.lock().active()
    }

    pub(crate) fn set_shared_modifiers(&self, modifiers: SharedModifiers) {
        self.lock().shared_modifiers = modifiers;
    }
}

impl<'a> Tuxel {
    fn lock(&'a self) -> MutexGuard<'a, TuxelInner> {
        self.inner
            .lock()
            .expect("TODO: handle thread panicking better than this")
    }
}

impl std::fmt::Display for Tuxel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content())?;
        Ok(())
    }
}

