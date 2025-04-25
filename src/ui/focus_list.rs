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
        if self.focused_index < self.items.len() - 1 {
            self.focused_index += 1;
        }
    }

    pub fn focus_previous(&mut self) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
        }
    }

    fn clamp_focused(&mut self) {
        if self.focused_index >= self.items.len() {
            self.focused_index = self.items.len().saturating_sub(1);
        }
    }

    pub fn add(&mut self, item: T) {
        if self.focused_index >= self.items.len() {
            self.items.push(item);
        } else {
            self.items.insert(self.focused_index + 1, item);
            self.focused_index += 1;
        }
    }

    pub fn remove(&mut self) {
        self.items.remove(self.focused_index);
        self.clamp_focused();
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

    pub fn remove_excess(&mut self, predicate: impl Fn(&T) -> bool) {
        for i in (0..self.items.len()).rev() {
            if self.items.len() == 1 {
                break;
            }

            if predicate(&self.items[i]) {
                self.items.remove(i);
            }
        }

        self.clamp_focused();
    }

    pub fn set_focused_index(&mut self, index: usize) {
        self.focused_index = index;
        self.clamp_focused();
    }

    pub fn focused_index(&self) -> usize {
        self.focused_index
    }
}
