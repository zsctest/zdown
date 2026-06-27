//! 文件树面板：浏览文件夹目录结构，打开 Markdown 文件。

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::editor_state::EditorState;
use i18n::I18n;

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
                n.depth > self.nodes[node_idx].depth && n.is_dir && n.path.starts_with(&node_path)
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

/// 渲染文件树面板。
pub fn show_file_tree_panel(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    editor_state: &mut EditorState,
    i18n: &I18n,
) {
    ui.horizontal(|ui| {
        if let Some(ref root) = state.root_path {
            ui.label(
                egui::RichText::new(root.display().to_string())
                    .size(13.0)
                    .strong(),
            );
        } else {
            ui.label(
                egui::RichText::new(i18n.t("file-tree-empty-hint"))
                    .size(13.0)
                    .weak(),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.root_path.is_some()
                && ui
                    .add(egui::Button::new("\u{1F504}").min_size(egui::vec2(20.0, 16.0)))
                    .on_hover_text(i18n.t("file-tree-refresh"))
                    .clicked()
            {
                state.refresh();
            }
        });
    });

    ui.separator();

    if state.root_path.is_none() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(i18n.t("file-tree-empty-detail"))
                    .size(12.0)
                    .weak(),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("file_tree_scroll")
        .show(ui, |ui| {
            let node_count = state.nodes.len();
            let mut ctx_menu: Option<FileContextMenu> = None;

            for i in 0..node_count {
                let resp = render_node(ui, state, i);
                if resp.clicked() {
                    handle_node_click(state, i, editor_state);
                }
                if resp.secondary_clicked() {
                    ctx_menu = Some(FileContextMenu {
                        node_index: i,
                        anchor: resp.rect.left_bottom(),
                    });
                }
            }

            state.context_menu = ctx_menu;
        });

    if let Some(ref menu) = state.context_menu.clone() {
        show_context_menu(ui, state, menu, editor_state, i18n);
    }

    if let Some((idx, ref mut new_name)) = state.renaming.clone() {
        let mut name = new_name.clone();
        show_inline_rename(ui, state, idx, &mut name, i18n);
        state.renaming = Some((idx, name));
    }

    if let Some((ref parent, ref mut name, ref mut focus)) = state.new_file_input.clone() {
        let mut n = name.clone();
        let mut f = *focus;
        show_new_file_input(ui, state, parent, &mut n, &mut f, editor_state, i18n);
        state.new_file_input = Some((parent.clone(), n, f));
    }
}

fn render_node(ui: &mut egui::Ui, state: &FileTreeState, idx: usize) -> egui::Response {
    let node = &state.nodes[idx];
    let indent = node.depth as f32 * 16.0;

    // 分配整行宽度的交互区域，使用 Sense::click() 确保点击能被检测到
    let row_height = 20.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    // 鼠标悬停时的高亮背景
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_gray(45));
    }

    // 在分配好的区域内渲染内容
    #[allow(deprecated)]
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(indent);

            if node.is_dir {
                let expanded = state.is_expanded(&node.path);
                let arrow = if expanded { "\u{25BC}" } else { "\u{25B6}" };
                ui.add(egui::Label::new(egui::RichText::new(arrow).size(11.0)));
            } else {
                ui.add_space(16.0);
            }

            let icon = if node.is_dir {
                if state.is_expanded(&node.path) {
                    "\u{1F4C2}"
                } else {
                    "\u{1F4C1}"
                }
            } else if FileTreeState::is_markdown(&node.path) {
                "\u{1F4DD}"
            } else {
                "\u{1F4C4}"
            };

            let name_color = if FileTreeState::is_markdown(&node.path) {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_gray(160)
            };

            ui.label(
                egui::RichText::new(format!("{icon} {}", node.name))
                    .color(name_color)
                    .size(13.0),
            );
        });
    });

    if FileTreeState::is_markdown(&node.path) {
        response
            .clone()
            .on_hover_text(node.path.display().to_string())
    } else {
        response
    }
}

fn handle_node_click(state: &mut FileTreeState, idx: usize, editor_state: &mut EditorState) {
    let node = &state.nodes[idx];
    if node.is_dir {
        state.toggle_expand(idx);
        return;
    }

    if FileTreeState::is_markdown(&node.path) {
        let _ = editor_state.open(&node.path);
    } else {
        editor_state.status_message = "This file type is not supported".to_string();
    }
}

fn show_context_menu(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    menu: &FileContextMenu,
    editor_state: &mut EditorState,
    i18n: &I18n,
) {
    let node_is_dir = state.nodes[menu.node_index].is_dir;
    let node_path = state.nodes[menu.node_index].path.clone();
    let node_name = state.nodes[menu.node_index].name.clone();
    let egui_ctx = ui.ctx().clone();

    egui::Area::new("file_tree_context_menu".into())
        .fixed_pos(menu.anchor)
        .order(egui::Order::Foreground)
        .show(&egui_ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);

                if node_is_dir {
                    if ui.button(i18n.t("file-tree-expand-all")).clicked() {
                        state.expand_all(menu.node_index);
                        state.context_menu = None;
                    }
                    if ui.button(i18n.t("file-tree-collapse-all")).clicked() {
                        state.collapse_all(menu.node_index);
                        state.context_menu = None;
                    }
                    ui.separator();
                    if ui.button(i18n.t("file-tree-new-file")).clicked() {
                        state.new_file_input = Some((node_path.clone(), String::new(), true));
                        state.context_menu = None;
                    }
                } else {
                    if ui.button(i18n.t("file-tree-open")).clicked() {
                        let path = node_path.clone();
                        let _ = editor_state.open(&path);
                        state.context_menu = None;
                    }
                }

                ui.separator();

                if ui.button(i18n.t("file-tree-rename")).clicked() {
                    state.renaming = Some((menu.node_index, node_name.clone()));
                    state.context_menu = None;
                }

                if ui.button(i18n.t("file-tree-delete")).clicked() {
                    let path = node_path.clone();
                    let is_dir = node_is_dir;
                    match if is_dir {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    } {
                        Ok(()) => {
                            state.refresh();
                            let _ = editor_state.close_tab_by_path(&path);
                        }
                        Err(e) => {
                            editor_state.status_message =
                                format!("{}: {e}", i18n.t("file-tree-delete-failed"));
                        }
                    }
                    state.context_menu = None;
                }

                ui.separator();

                if ui.button(i18n.t("file-tree-reveal")).clicked() {
                    let reveal_path = if node_is_dir {
                        node_path.clone()
                    } else {
                        node_path
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_default()
                    };
                    let _ = open::that(&reveal_path);
                    state.context_menu = None;
                }
            });
        });

    if egui_ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || egui_ctx.input(|i| i.pointer.primary_clicked())
    {
        state.context_menu = None;
    }
}

fn show_inline_rename(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    idx: usize,
    new_name: &mut String,
    i18n: &I18n,
) {
    let egui_ctx = ui.ctx().clone();

    egui::Window::new(i18n.t("file-tree-rename"))
        .collapsible(false)
        .resizable(false)
        .min_size(egui::vec2(300.0, 80.0))
        .show(&egui_ctx, |ui| {
            ui.label(i18n.t("file-tree-rename-prompt"));
            ui.add(egui::TextEdit::singleline(new_name).desired_width(260.0));

            ui.horizontal(|ui| {
                if ui.button("\u{2713}").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    let old_path = state.nodes[idx].path.clone();
                    let parent = old_path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_default();
                    let new_path = parent.join(new_name.as_str());

                    if !new_path.exists() {
                        let _ = std::fs::rename(&old_path, &new_path);
                        state.refresh();
                    }
                    state.renaming = None;
                }
                if ui.button("\u{2717}").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    state.renaming = None;
                }
            });
        });
}

fn show_new_file_input(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    parent: &Path,
    name: &mut String,
    focus: &mut bool,
    editor_state: &mut EditorState,
    i18n: &I18n,
) {
    let egui_ctx = ui.ctx().clone();

    egui::Window::new(i18n.t("file-tree-new-file"))
        .collapsible(false)
        .resizable(false)
        .min_size(egui::vec2(300.0, 80.0))
        .show(&egui_ctx, |ui| {
            let id = egui::Id::new("new_file_name_input");
            if *focus {
                egui_ctx.memory_mut(|m| m.request_focus(id));
                *focus = false;
            }

            ui.add(
                egui::TextEdit::singleline(name)
                    .id(id)
                    .hint_text("filename.md")
                    .desired_width(260.0),
            );

            ui.horizontal(|ui| {
                if ui.button("\u{2713}").clicked()
                    || (ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty())
                {
                    let mut filename = name.clone();
                    if !filename.ends_with(".md") {
                        filename.push_str(".md");
                    }
                    let new_path = parent.join(&filename);
                    match std::fs::write(&new_path, "") {
                        Ok(()) => {
                            state.refresh();
                            let _ = editor_state.open(&new_path);
                        }
                        Err(e) => {
                            editor_state.status_message = format!("Failed to create file: {e}");
                        }
                    }
                    state.new_file_input = None;
                }
                if ui.button("\u{2717}").clicked() {
                    state.new_file_input = None;
                }
            });
        });
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
