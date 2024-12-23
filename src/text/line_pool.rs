pub type Line = Vec<char>;

pub struct LinePool {
    available: Vec<Line>,
}

impl LinePool {
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
        }
    }

    pub fn pop(&mut self) -> Line {
        self.available.pop().unwrap_or_default()
    }

    pub fn push(&mut self, mut line: Line) {
        line.clear();
        self.available.push(line);
    }
}
