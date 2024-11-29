// Like a Vec, but the indices of item in the list are always preserved.
pub struct SlotList<T> {
    items: Vec<Option<T>>,
}

impl<T> SlotList<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: T) -> usize {
        let mut index = None;

        for i in 0..self.items.len() {
            if self.items[i].is_none() {
                index = Some(i);
                break;
            }
        }

        if let Some(index) = index {
            self.items[index] = Some(item);
            index
        } else {
            self.items.push(Some(item));
            self.items.len() - 1
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.items.push(None);
        self.items.swap_remove(index)
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index).and_then(|item| item.as_ref())
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(index).and_then(|item| item.as_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Option<T>> {
        self.items.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Option<T>> {
        self.items.iter_mut()
    }
}
