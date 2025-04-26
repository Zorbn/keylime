pub struct TempBuffer<T> {
    data: Option<Vec<T>>,
}

impl<T> TempBuffer<T> {
    pub fn new() -> Self {
        Self {
            data: Some(Vec::new()),
        }
    }

    pub fn get_mut(&mut self) -> &mut Vec<T> {
        let data = self.data.as_mut().unwrap();
        data.clear();

        data
    }
}

pub struct TempString {
    data: Option<String>,
}

impl TempString {
    pub fn new() -> Self {
        Self {
            data: Some(String::new()),
        }
    }

    pub fn get_mut(&mut self) -> &mut String {
        let data = self.data.as_mut().unwrap();
        data.clear();

        data
    }

    pub fn take_mut(&mut self) -> String {
        let mut data = self.data.take().unwrap();
        data.clear();

        data
    }

    pub fn replace(&mut self, data: String) {
        assert!(self.data.is_none());

        self.data = Some(data);
    }
}
