pub struct LinePool {
    available: Vec<String>,
}

impl LinePool {
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
        }
    }

    pub fn pop(&mut self) -> String {
        self.available.pop().unwrap_or_default()
    }

    pub fn push(&mut self, mut line: String) {
        line.clear();
        self.available.push(line);
    }
}
