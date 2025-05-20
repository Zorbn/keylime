use crate::pool::{Pooled, STRING_POOL};

use super::grapheme::CharCursor;

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

    pub fn insert(&mut self, text: &str) {
        self.insert_at_node(0, text);
    }

    pub fn traverse(&self, prefix: &str, mut result_fn: impl FnMut(Pooled<String>)) {
        self.traverse_with_prefix_at_node(0, prefix, prefix, &mut result_fn);
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.data.clear();

        let root = self.new_node();
        self.nodes.push(root);
    }

    fn insert_at_node(&mut self, mut index: usize, text: &str) {
        let mut char_cursor = CharCursor::new(0, text.len());

        while let Some(c) = text[char_cursor.index()..].chars().nth(0) {
            index = self.get_or_add_child(index, c);
            char_cursor.next_boundary(text);
        }

        self.nodes[index].is_terminal = true;
    }

    // Traverses nodes that match a specific prefix.
    fn traverse_with_prefix_at_node(
        &self,
        index: usize,
        prefix: &str,
        remaining: &str,
        result_fn: &mut impl FnMut(Pooled<String>),
    ) {
        let node = &self.nodes[index];

        if remaining.is_empty() {
            for i in 0..node.len {
                let child = &self.data[node.start + i];

                let mut new_prefix = STRING_POOL.new_item();
                new_prefix.push_str(prefix);
                new_prefix.push(child.0);

                self.traverse_at_node(child.1, new_prefix, result_fn);
            }

            return;
        }

        let mut char_cursor = CharCursor::new(0, remaining.len());
        char_cursor.next_boundary(remaining);

        let c = remaining.chars().nth(0).unwrap();
        let remaining = &remaining[char_cursor.index()..];

        for i in 0..node.len {
            let child = &self.data[node.start + i];

            if child.0 == c {
                self.traverse_with_prefix_at_node(child.1, prefix, remaining, result_fn);
            }
        }
    }

    // Traverses all nodes.
    fn traverse_at_node(
        &self,
        index: usize,
        prefix: Pooled<String>,
        result_fn: &mut impl FnMut(Pooled<String>),
    ) {
        let node = &self.nodes[index];

        let prefix = if node.is_terminal {
            let mut new_prefix = STRING_POOL.new_item();
            new_prefix.push_str(&prefix);

            result_fn(prefix);

            new_prefix
        } else {
            prefix
        };

        for child in &self.data[node.start..node.start + node.len] {
            let mut new_prefix = STRING_POOL.new_item();
            new_prefix.push_str(&prefix);
            new_prefix.push(child.0);

            self.traverse_at_node(child.1, new_prefix, result_fn);
        }
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
