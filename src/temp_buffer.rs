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

pub struct TempString {
    data: String,
}

impl TempString {
    pub fn new() -> Self {
        Self {
            data: String::new(),
        }
    }

    pub fn get_mut(&mut self) -> &mut String {
        self.data.clear();

        &mut self.data
    }
}
