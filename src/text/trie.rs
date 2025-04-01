use super::line_pool::LinePool;

struct TrieNode {
    start: usize,
    len: usize,
    capacity: usize,
    is_terminal: bool,
}

pub struct Trie {
    nodes: Vec<TrieNode>,
    data: Vec<(char, usize)>,
}

impl Trie {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            data: Vec::new(),
        }
    }

    pub fn insert(&mut self, chars: &[char]) {
        self.insert_at_node(0, chars);
    }

    pub fn traverse(&self, prefix: &[char], results: &mut Vec<String>, result_pool: &mut LinePool) {
        self.traverse_with_prefix_at_node(0, prefix, prefix, results, result_pool);
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.data.clear();

        let root = self.new_node();
        self.nodes.push(root);
    }

    fn insert_at_node(&mut self, mut index: usize, mut chars: &[char]) {
        while !chars.is_empty() {
            let c = chars[0];
            let remaining = &chars[1..];

            index = self.get_or_add_child(index, c);
            chars = remaining;
        }

        self.nodes[index].is_terminal = true;
    }

    // Traverses nodes that match a specific prefix.
    fn traverse_with_prefix_at_node(
        &self,
        index: usize,
        prefix: &[char],
        remaining: &[char],
        results: &mut Vec<String>,
        result_pool: &mut LinePool,
    ) {
        // let node = &self.nodes[index];

        // if remaining.is_empty() {
        //     for i in 0..node.len {
        //         let child = &self.data[node.start + i];

        //         let mut new_prefix = result_pool.pop();
        //         new_prefix.extend_from_slice(prefix);
        //         new_prefix.push(child.0);

        //         self.traverse_at_node(child.1, new_prefix, results, result_pool);
        //     }

        //     return;
        // }

        // let c = remaining[0];
        // let remaining = &remaining[1..];

        // for i in 0..node.len {
        //     let child = &self.data[node.start + i];

        //     if child.0 == c {
        //         self.traverse_with_prefix_at_node(child.1, prefix, remaining, results, result_pool);
        //     }
        // }
    }

    // Traverses all nodes.
    fn traverse_at_node(
        &self,
        index: usize,
        prefix: String, // TODO: Should this be &str?
        results: &mut Vec<String>,
        result_pool: &mut LinePool,
    ) {
        // let node = &self.nodes[index];

        // let prefix = if node.is_terminal {
        //     let mut new_prefix = result_pool.pop();
        //     new_prefix.extend_from_slice(&prefix);

        //     results.push(prefix);

        //     new_prefix
        // } else {
        //     prefix
        // };

        // for child in &self.data[node.start..node.start + node.len] {
        //     let mut new_prefix = result_pool.pop();
        //     new_prefix.extend_from_slice(&prefix);
        //     new_prefix.push(child.0);

        //     self.traverse_at_node(child.1, new_prefix, results, result_pool);
        // }

        // result_pool.push(prefix);
    }

    fn new_node(&mut self) -> TrieNode {
        let start = self.data.len();
        let capacity = 4;

        self.data.resize(start + capacity, (' ', 0));

        TrieNode {
            start,
            len: 0,
            capacity,
            is_terminal: false,
        }
    }

    fn add_child_to_node(&mut self, index: usize, key: char) -> usize {
        let child_node = self.new_node();
        let child_index = self.nodes.len();
        self.nodes.push(child_node);

        let node = &mut self.nodes[index];

        if node.len >= node.capacity {
            let new_start = self.data.len();
            let new_capacity = node.capacity * 2;

            self.data.resize(new_start + new_capacity, (' ', 0));
            self.data
                .copy_within(node.start..node.start + node.capacity, new_start);

            node.start = new_start;
            node.capacity = new_capacity;
        }

        self.data[node.start + node.len] = (key, child_index);
        node.len += 1;

        child_index
    }

    fn get_or_add_child(&mut self, index: usize, key: char) -> usize {
        let node = &self.nodes[index];

        for child in &self.data[node.start..node.start + node.len] {
            if child.0 == key {
                return child.1;
            }
        }

        self.add_child_to_node(index, key)
    }
}
