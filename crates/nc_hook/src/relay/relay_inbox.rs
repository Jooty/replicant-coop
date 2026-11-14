use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relayed<M> {
    pub from: u8,
    pub message: M,
}

#[derive(Debug)]
pub struct RelayInbox<M> {
    items: Mutex<VecDeque<Relayed<M>>>,
    capacity: usize,
}

impl<M> RelayInbox<M> {
    pub const fn new(capacity: usize) -> Self {
        Self {
            items: Mutex::new(VecDeque::new()),
            capacity,
        }
    }

    pub fn push(&self, from: u8, message: M) {
        let mut items = self.items.lock().unwrap_or_else(PoisonError::into_inner);
        if items.len() >= self.capacity {
            items.pop_front();
        }
        items.push_back(Relayed { from, message });
    }

    /// Removes the waiting messages `wanted` accepts, oldest first, without their senders.
    pub fn take_matching(&self, wanted: impl Fn(&M) -> bool) -> Vec<M> {
        self.take_matching_from(wanted)
            .into_iter()
            .map(|relayed| relayed.message)
            .collect()
    }

    pub fn take_matching_from(&self, wanted: impl Fn(&M) -> bool) -> Vec<Relayed<M>> {
        let mut items = self.items.lock().unwrap_or_else(PoisonError::into_inner);
        let (taken, kept): (VecDeque<Relayed<M>>, VecDeque<Relayed<M>>) = items
            .drain(..)
            .partition(|relayed| wanted(&relayed.message));
        *items = kept;
        taken.into()
    }

    pub fn clear(&self) {
        self.items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    pub fn len(&self) -> usize {
        self.items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
