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
    data: Vec<String>,
}

impl TempString {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn get_mut(&mut self) -> &mut String {
        if self.data.is_empty() {
            self.data.push(String::new());
        }

        let data = self.data.last_mut().unwrap();
        data.clear();

        data
    }

    pub fn pop(&mut self) -> String {
        let mut data = self.data.pop().unwrap_or_default();
        data.clear();

        data
    }

    pub fn push(&mut self, data: String) {
        self.data.push(data);
    }
}
