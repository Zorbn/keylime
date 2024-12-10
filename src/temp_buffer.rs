pub struct TempBuffer<T> {
    data: Vec<T>,
}

impl<T> TempBuffer<T> {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn get_mut(&mut self) -> &mut Vec<T> {
        self.data.clear();

        &mut self.data
    }
}
