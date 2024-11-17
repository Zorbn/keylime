use std::{
    fs::{read_dir, ReadDir},
    io,
    mem::swap,
    ops::Range,
    path::{Path, PathBuf},
    str::Chars,
};

use crate::{gfx::Gfx, rect::Rect, theme::Theme};

struct VisualFileTreeNode {
    index: usize,
    bounds: Rect,
}

#[derive(Debug)]
enum FileTreeNode {
    File {
        path: PathBuf,
    },
    Folder {
        path: PathBuf,
        children_range: Option<Range<usize>>,
        is_open: bool,
    },
}

pub struct FileTree {
    previous_nodes: Vec<FileTreeNode>,
    nodes: Vec<FileTreeNode>,
    visual_nodes: Vec<VisualFileTreeNode>,
    bounds: Rect,
    is_layout_dirty: bool,
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            previous_nodes: Vec::new(),
            nodes: Vec::new(),
            visual_nodes: Vec::new(),
            bounds: Rect::zero(),
            is_layout_dirty: true,
        }
    }

    pub fn layout(&mut self, bounds: Rect, gfx: &Gfx) {
        if !self.is_layout_dirty {
            return;
        }

        self.is_layout_dirty = false;

        self.bounds = Rect::new(bounds.x, bounds.y, gfx.file_tree_width(), bounds.height);
        self.visual_nodes.clear();

        let mut y = self.bounds.y;

        self.layout_node(0, self.bounds.x, &mut y, gfx);
    }

    pub fn layout_node(&mut self, index: usize, x: f32, y: &mut f32, gfx: &Gfx) {
        const FOLDER_PREFIX_LEN: isize = 2;

        match &self.nodes[index] {
            FileTreeNode::File { path } => {
                let width = Gfx::measure_text(Self::path_chars(path));

                self.visual_nodes.push(VisualFileTreeNode {
                    index,
                    bounds: Rect::new(x, *y, width as f32 * gfx.glyph_width(), gfx.line_height()),
                });
            }
            FileTreeNode::Folder {
                path,
                children_range,
                ..
            } => {
                let width = Gfx::measure_text(Self::path_chars(path)) + FOLDER_PREFIX_LEN;

                self.visual_nodes.push(VisualFileTreeNode {
                    index,
                    bounds: Rect::new(x, *y, width as f32 * gfx.glyph_width(), gfx.line_height()),
                });

                if let Some(children_range) = children_range {
                    for i in children_range.clone() {
                        *y += gfx.glyph_height();
                        self.layout_node(i, x + gfx.glyph_width(), y, gfx);
                    }
                }
            }
        }
    }

    pub fn draw(&self, theme: &Theme, gfx: &mut Gfx) {
        gfx.begin(Some(self.bounds));

        for node in &self.visual_nodes {
            let (prefix, text) = match &self.nodes[node.index] {
                FileTreeNode::File { path } => (None, Self::path_chars(path)),
                FileTreeNode::Folder { path, is_open, .. } => {
                    let prefix = if *is_open { "- ".chars() } else { "+ ".chars() };

                    (Some(prefix), Self::path_chars(path))
                }
            };

            let prefix_width = if let Some(prefix) = prefix {
                gfx.add_text(prefix, node.bounds.x, node.bounds.y, &theme.normal)
            } else {
                0
            };

            gfx.add_text(
                text,
                node.bounds.x + prefix_width as f32 * gfx.glyph_width(),
                node.bounds.y,
                &theme.normal,
            );
        }

        gfx.end();
    }

    fn path_chars(path: &Path) -> Chars<'_> {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("???")
            .chars()
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    pub fn load(&mut self, root: Option<PathBuf>) -> io::Result<()> {
        self.is_layout_dirty = true;

        swap(&mut self.nodes, &mut self.previous_nodes);
        self.nodes.clear();

        let previous_root =
            if let Some(FileTreeNode::Folder { path, .. }) = self.previous_nodes.first() {
                Some(path.clone())
            } else {
                None
            };

        if root.is_some() && root != previous_root {
            self.previous_nodes.clear();
        }

        let Some(root) = root
            .and_then(|root| root.canonicalize().ok())
            .or(previous_root)
        else {
            return Ok(());
        };

        let entries = read_dir(&root)?;

        let root_index = self.nodes.len();
        self.nodes.push(FileTreeNode::Folder {
            path: root,
            children_range: None,
            is_open: true,
        });

        self.handle_folder(entries, root_index)?;

        Ok(())
    }

    pub fn open(&mut self, index: usize) -> io::Result<()> {
        if let Some(FileTreeNode::Folder {
            path,
            is_open: false,
            ..
        }) = self.nodes.get(index)
        {
            self.is_layout_dirty = true;

            let entries = read_dir(path)?;
            self.handle_folder(entries, index)?;
        }

        Ok(())
    }

    fn handle_folder(&mut self, entries: ReadDir, parent_index: usize) -> io::Result<()> {
        let start_index = self.nodes.len();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            let node = if path.is_dir() {
                FileTreeNode::Folder {
                    path,
                    children_range: None,
                    is_open: false,
                }
            } else {
                FileTreeNode::File { path }
            };

            self.nodes.push(node);
        }

        let end_index = self.nodes.len();

        if let FileTreeNode::Folder {
            children_range,
            is_open,
            ..
        } = &mut self.nodes[parent_index]
        {
            *children_range = Some(start_index..end_index);
            *is_open = true;
        }

        for i in start_index..end_index {
            let FileTreeNode::Folder { path, .. } = &self.nodes[i] else {
                continue;
            };

            let Some(FileTreeNode::Folder {
                path: previous_path,
                is_open: was_open,
                ..
            }) = self.previous_nodes.get(i)
            else {
                continue;
            };

            if *was_open && path == previous_path {
                let entries = read_dir(path)?;
                self.handle_folder(entries, i)?;
            }
        }

        Ok(())
    }

    pub fn test_print(&self, index: usize, indentation: usize) {
        match &self.nodes[index] {
            FileTreeNode::File { path } => {
                for _ in 0..indentation {
                    print!("-")
                }

                println!(" {:?}", path);
            }
            FileTreeNode::Folder {
                path,
                children_range,
                is_open,
            } => {
                for _ in 0..indentation {
                    print!("-")
                }

                println!(" {} {:?}", if *is_open { 'v' } else { '>' }, path);

                if let Some(children_range) = children_range {
                    for i in children_range.clone() {
                        self.test_print(i, indentation + 1);
                    }
                }
            }
        }
    }
}
