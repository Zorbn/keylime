use std::{cmp::Ordering, vec::Drain};

pub struct FocusList<T> {
    items: Vec<T>,
    focused_index: usize,
}

impl<T> FocusList<T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            focused_index: 0,
        }
    }

    pub fn focus_next(&mut self) {
        if self.focused_index < self.items.len().saturating_sub(1) {
            self.focused_index += 1;
        } else {
            self.focused_index = 0;
        }
    }

    pub fn focus_previous(&mut self) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
        } else {
            self.focused_index = self.items.len().saturating_sub(1);
        }
    }

    fn clamp_focused(&mut self) {
        self.focused_index = self.focused_index.min(self.items.len().saturating_sub(1));
    }

    pub fn add(&mut self, item: T) {
        if self.focused_index >= self.items.len() {
            self.items.push(item);
        } else {
            self.items.insert(self.focused_index + 1, item);
            self.focused_index += 1;
        }
    }

    pub fn insert(&mut self, index: usize, item: T) {
        if index < self.items.len() && self.focused_index >= index {
            self.focused_index += 1;
        }

        self.items.insert(index, item);
    }

    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn append(&mut self, other: &mut Vec<T>) {
        self.items.append(other);
    }

    pub fn remove(&mut self) -> Option<T> {
        let item =
            (self.focused_index < self.items.len()).then(|| self.items.remove(self.focused_index));

        self.clamp_focused();

        item
    }

    pub fn drain(&mut self) -> Drain<T> {
        self.focused_index = 0;

        self.items.drain(..)
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        if self.focused_index == a {
            self.focused_index = b;
        } else if self.focused_index == b {
            self.focused_index = a;
        }

        self.items.swap(a, b);
    }

    pub fn sort_by(&mut self, compare: impl FnMut(&T, &T) -> Ordering) {
        self.items.sort_by(compare);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn get_focused(&self) -> Option<&T> {
        self.items.get(self.focused_index)
    }

    pub fn get_focused_mut(&mut self) -> Option<&mut T> {
        self.items.get_mut(self.focused_index)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn set_focused_index(&mut self, index: usize) {
        self.focused_index = index;
        self.clamp_focused();
    }

    pub fn focused_index(&self) -> usize {
        self.focused_index
    }
}
