//! 文件树面板：浏览文件夹目录结构，打开 Markdown 文件。

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 文件树中的一个节点。
#[derive(Debug, Clone)]
pub struct FileTreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
}

/// 文件树面板的完整状态。
#[derive(Debug, Clone, Default)]
pub struct FileTreeState {
    pub root_path: Option<PathBuf>,
    pub nodes: Vec<FileTreeNode>,
    expanded: BTreeSet<PathBuf>,
    pub context_menu: Option<FileContextMenu>,
    pub renaming: Option<(usize, String)>,
    pub new_file_input: Option<(PathBuf, String, bool)>,
}

impl FileTreeState {
    pub fn open_folder(&mut self, path: &Path) {
        self.root_path = Some(path.to_path_buf());
        self.nodes.clear();
        self.expanded.clear();
        self.context_menu = None;
        self.renaming = None;
        self.new_file_input = None;

        let root_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        self.nodes.push(FileTreeNode {
            name: root_name,
            path: path.to_path_buf(),
            is_dir: true,
            depth: 0,
        });

        self.expanded.insert(path.to_path_buf());
        self.load_children(0);
    }

    fn load_children(&mut self, parent_idx: usize) {
        let parent_path = self.nodes[parent_idx].path.clone();
        let parent_depth = self.nodes[parent_idx].depth;

        let dir_entries = match std::fs::read_dir(&parent_path) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("读取目录失败 {}: {e}", parent_path.display());
                return;
            }
        };

        let mut children: Vec<FileTreeNode> = Vec::new();
        for entry in dir_entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            children.push(FileTreeNode {
                name,
                path,
                is_dir,
                depth: parent_depth + 1,
            });
        }

        children.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let insert_pos = self.find_insert_pos(parent_idx);
        self.nodes.splice(insert_pos..insert_pos, children);
    }

    fn find_insert_pos(&self, parent_idx: usize) -> usize {
        let parent_depth = self.nodes[parent_idx].depth;
        let mut pos = parent_idx + 1;
        while pos < self.nodes.len() && self.nodes[pos].depth > parent_depth {
            pos += 1;
        }
        pos
    }

    pub fn toggle_expand(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];
        if !node.is_dir {
            return;
        }

        if self.expanded.contains(&node.path) {
            self.expanded.remove(&node.path);
            self.remove_descendants(node_idx);
        } else {
            self.expanded.insert(node.path.clone());
            self.load_children(node_idx);
        }
    }

    fn remove_descendants(&mut self, parent_idx: usize) {
        let parent_depth = self.nodes[parent_idx].depth;
        let mut remove_start = parent_idx + 1;
        while remove_start < self.nodes.len() && self.nodes[remove_start].depth > parent_depth {
            remove_start += 1;
        }
        let remove_count = remove_start - parent_idx - 1;
        if remove_count > 0 {
            for i in parent_idx + 1..remove_start {
                self.expanded.remove(&self.nodes[i].path);
            }
            self.nodes.drain(parent_idx + 1..remove_start);
        }
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    pub fn find_by_path(&self, path: &Path) -> Option<usize> {
        self.nodes.iter().position(|n| n.path == path)
    }

    pub fn refresh(&mut self) {
        if let Some(root) = self.root_path.clone() {
            self.open_folder(&root);
        }
    }

    pub fn expand_all(&mut self, node_idx: usize) {
        let node_path = self.nodes[node_idx].path.clone();
        if !self.nodes[node_idx].is_dir {
            return;
        }
        if !self.expanded.contains(&node_path) {
            self.expanded.insert(node_path.clone());
            self.load_children(node_idx);
        }
        let child_paths: Vec<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| {
                n.depth > self.nodes[node_idx].depth
                    && n.is_dir
                    && n.path.starts_with(&node_path)
            })
            .map(|n| n.path.clone())
            .collect();

        for path in child_paths {
            if let Some(idx) = self.find_by_path(&path) {
                self.expand_all(idx);
            }
        }
    }

    pub fn collapse_all(&mut self, node_idx: usize) {
        let node_path = self.nodes[node_idx].path.clone();
        if !self.nodes[node_idx].is_dir {
            return;
        }
        let descendant_dirs: Vec<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| {
                n.is_dir && n.depth > self.nodes[node_idx].depth && n.path.starts_with(&node_path)
            })
            .map(|n| n.path.clone())
            .collect();

        for path in &descendant_dirs {
            self.expanded.remove(path);
        }
        self.expanded.remove(&node_path);
        self.remove_descendants(node_idx);
    }

    pub fn is_markdown(path: &Path) -> bool {
        path.extension()
            .map(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false)
    }
}

/// 右键菜单状态。
#[derive(Debug, Clone)]
pub struct FileContextMenu {
    pub node_index: usize,
    pub anchor: egui::Pos2,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("create temp dir");
        let root = dir.path().to_path_buf();
        fs::create_dir(root.join("docs")).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join(".hidden")).unwrap();
        fs::write(root.join("README.md"), "# Hello").unwrap();
        fs::write(root.join("docs/guide.md"), "## Guide").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join(".hidden/secret.md"), "secret").unwrap();
        (dir, root)
    }

    #[test]
    fn open_folder_builds_tree() {
        let (_dir, root) = setup_test_dir();
        let mut state = FileTreeState::default();
        state.open_folder(&root);

        assert!(state.root_path.is_some());
        assert!(
            state.nodes.len() >= 4,
            "expected at least root + 3 visible children"
        );
        let readme = state.nodes.iter().find(|n| n.name == "README.md");
        assert!(readme.is_some(), "README.md should exist");
        assert!(!readme.unwrap().is_dir);
        let hidden = state.nodes.iter().find(|n| n.name == ".hidden");
        assert!(hidden.is_none(), ".hidden should be skipped");
    }

    #[test]
    fn toggle_expand_directory() {
        let (_dir, root) = setup_test_dir();
        let mut state = FileTreeState::default();
        state.open_folder(&root);

        let docs_idx = state
            .nodes
            .iter()
            .position(|n| n.name == "docs" && n.is_dir)
            .expect("docs dir should exist");

        assert!(!state.is_expanded(&state.nodes[docs_idx].path));

        state.toggle_expand(docs_idx);
        assert!(state.is_expanded(&state.nodes[docs_idx].path));
        let guide = state.nodes.iter().find(|n| n.name == "guide.md");
        assert!(guide.is_some(), "guide.md should appear after expand");

        state.toggle_expand(docs_idx);
        assert!(!state.is_expanded(&state.nodes[docs_idx].path));
    }

    #[test]
    fn empty_folder_has_only_root() {
        let dir = TempDir::new().expect("create temp dir");
        let mut state = FileTreeState::default();
        state.open_folder(dir.path());
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.nodes[0].depth, 0);
    }

    #[test]
    fn nodes_sorted_dirs_first() {
        let (_dir, root) = setup_test_dir();
        let mut state = FileTreeState::default();
        state.open_folder(&root);

        let root_children: Vec<&FileTreeNode> =
            state.nodes.iter().filter(|n| n.depth == 1).collect();

        let first_file_pos = root_children.iter().position(|n| !n.is_dir);
        if let Some(pos) = first_file_pos {
            for dir_node in &root_children[..pos] {
                assert!(dir_node.is_dir, "{} should be a directory", dir_node.name);
            }
            for file_node in &root_children[pos..] {
                assert!(!file_node.is_dir, "{} should be a file", file_node.name);
            }
        }
    }
}
