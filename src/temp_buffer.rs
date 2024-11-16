use std::ops::{Deref, DerefMut};

pub struct TempBuffer<T> {
    data: Vec<T>,
}

impl<T> TempBuffer<T> {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn get_mut(&mut self) -> TempBufferHandle<T> {
        TempBufferHandle {
            data: &mut self.data,
        }
    }
}

pub struct TempBufferHandle<'a, T> {
    data: &'a mut Vec<T>,
}

impl<'a, T> Drop for TempBufferHandle<'a, T> {
    fn drop(&mut self) {
        self.data.clear();
    }
}

impl<'a, T> Deref for TempBufferHandle<'a, T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> DerefMut for TempBufferHandle<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
