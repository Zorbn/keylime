#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotId {
    index: usize,
    generation: usize,
}

impl SlotId {
    pub const ZERO: Self = Self::new(0, 0);

    const fn new(index: usize, generation: usize) -> Self {
        Self { index, generation }
    }
}

struct Slot<T> {
    item: Option<T>,
    generation: usize,
}

// Like a Vec, but the indices of item in the list are always preserved.
pub struct SlotList<T> {
    slots: Vec<Slot<T>>,
    unused_slot_indices: Vec<usize>,
}

impl<T> SlotList<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            unused_slot_indices: Vec::new(),
        }
    }

    pub fn add(&mut self, item: T) -> SlotId {
        if let Some(index) = self.unused_slot_indices.pop() {
            let slot = &mut self.slots[index];
            slot.item = Some(item);

            SlotId {
                index,
                generation: slot.generation,
            }
        } else {
            let index = self.slots.len();
            let generation = 0;

            self.slots.push(Slot {
                item: Some(item),
                generation,
            });

            SlotId { index, generation }
        }
    }

    pub fn remove(&mut self, id: SlotId) -> Option<T> {
        if id.index > self.slots.len() {
            return None;
        }

        let slot = &mut self.slots[id.index];

        if slot.generation != id.generation {
            return None;
        }

        let item = slot.item.take();
        slot.generation += 1;

        self.unused_slot_indices.push(id.index);

        item
    }

    pub fn get(&self, id: SlotId) -> Option<&T> {
        self.slots
            .get(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.item.as_ref())
    }

    pub fn get_mut(&mut self, id: SlotId) -> Option<&mut T> {
        self.slots
            .get_mut(id.index)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.item.as_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().flat_map(|slot| &slot.item)
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().flat_map(|slot| &mut slot.item)
    }

    pub fn enumerate(&self) -> impl Iterator<Item = (SlotId, &T)> {
        self.slots.iter().enumerate().flat_map(|(index, slot)| {
            slot.item
                .as_ref()
                .map(|item| (SlotId::new(index, slot.generation), item))
        })
    }

    pub fn enumerate_mut(&mut self) -> impl Iterator<Item = (SlotId, &mut T)> {
        self.slots.iter_mut().enumerate().flat_map(|(index, slot)| {
            slot.item
                .as_mut()
                .map(|item| (SlotId::new(index, slot.generation), item))
        })
    }
}
