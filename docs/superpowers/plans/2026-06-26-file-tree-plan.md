# 文件树面板 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在左侧面板新增文件树浏览器，支持打开文件夹、浏览目录、右键操作，实现类似 VS Code Explorer 的体验。

**架构：** 新增 `file_tree.rs` 模块管理树状态和 UI 渲染；在 `workspace::dialog` 新增 `pick_folder`；在菜单新增"打开文件夹"项；在 ZdownApp 集成拖拽和布局。节点扁平存储，按需展开子目录。

**技术栈：** Rust + egui + rfd（已有）+ std::fs

---

### 任务 1：新增 `pick_folder` 对话框函数

**文件：**
- 修改：`crates/workspace/src/dialog.rs:1-55`

- [ ] **步骤 1：在 `pick_open_image` 之后新增 `pick_folder` 函数**

```rust
/// 弹出选择文件夹对话框。用户取消或环境不支持时返回 None。
pub fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .pick_folder()
}
```

- [ ] **步骤 2：在测试模块中新增对应的 ignored 测试**

在 `mod tests` 块末尾、`pick_open_image_does_not_panic` 测试之后新增：

```rust
#[test]
#[ignore = "需要手动在桌面环境验证对话框弹窗"]
fn pick_folder_does_not_panic() {
    let _ = pick_folder("Open Folder");
}
```

- [ ] **步骤 3：运行测试验证不 panic**

```bash
cargo test -p workspace -- --ignored --test-threads=1
```

> 如果环境无 DISPLAY，均返回 None 不 panic。手动有桌面时弹窗正常。

- [ ] **步骤 4：Commit**

```bash
git add crates/workspace/src/dialog.rs
git commit -m "feat(workspace): add pick_folder dialog function"
```

---

### 任务 2：新增 i18n 翻译键

**文件：**
- 修改：`crates/i18n/locales/zh-CN/menu.ftl`
- 修改：`crates/i18n/locales/en-US/menu.ftl`
- 创建：`crates/i18n/locales/zh-CN/file-tree.ftl`
- 创建：`crates/i18n/locales/en-US/file-tree.ftl`
- 修改：`crates/i18n/src/resource.rs:11-34`

- [ ] **步骤 1：在中文菜单 FTL 中添加"打开文件夹"菜单项**

在 `crates/i18n/locales/zh-CN/menu.ftl` 的 `menu-file-open` 行之后插入：

```
menu-file-open-folder = 打开文件夹...
```

- [ ] **步骤 2：在英文菜单 FTL 中添加**

在 `crates/i18n/locales/en-US/menu.ftl` 的 `menu-file-open` 行之后插入：

```
menu-file-open-folder = Open Folder...
```

- [ ] **步骤 3：创建中文文件树 FTL**

创建 `crates/i18n/locales/zh-CN/file-tree.ftl`：

```ftl
# 文件树面板 - 中文

file-tree-empty-hint = 打开文件夹以浏览文件
file-tree-empty-detail = 文件 → 打开文件夹 或拖拽文件夹到窗口
file-tree-open = 打开
file-tree-rename = 重命名
file-tree-delete = 删除
file-tree-reveal = 在文件管理器中显示
file-tree-new-file = 新建 Markdown 文件
file-tree-expand-all = 展开全部
file-tree-collapse-all = 折叠全部
file-tree-refresh = 刷新
file-tree-unsupported = 此文件类型不支持直接打开
file-tree-confirm-delete = 确认删除
file-tree-confirm-delete-body = 确定要删除 "{ $name }" 吗？此操作不可撤销。
file-tree-rename-prompt = 请输入新名称：
file-tree-delete-failed = 删除失败: { $error }
```

- [ ] **步骤 4：创建英文文件树 FTL**

创建 `crates/i18n/locales/en-US/file-tree.ftl`：

```ftl
# File Tree Panel - English

file-tree-empty-hint = Open a folder to browse files
file-tree-empty-detail = File → Open Folder or drag a folder into the window
file-tree-open = Open
file-tree-rename = Rename
file-tree-delete = Delete
file-tree-reveal = Reveal in File Manager
file-tree-new-file = New Markdown File
file-tree-expand-all = Expand All
file-tree-collapse-all = Collapse All
file-tree-refresh = Refresh
file-tree-unsupported = This file type is not supported
file-tree-confirm-delete = Confirm Delete
file-tree-confirm-delete-body = Are you sure you want to delete "{ $name }"? This cannot be undone.
file-tree-rename-prompt = Enter new name:
file-tree-delete-failed = Delete failed: { $error }
```

- [ ] **步骤 5：在 resource.rs 中注册新 FTL 文件**

修改 `crates/i18n/src/resource.rs`：

在 `create_bundle_zh_cn` 函数中，`add_resource` 调用列表末尾（`actions.ftl` 之后）新增：

```rust
add_resource(&mut bundle, include_str!("../locales/zh-CN/file-tree.ftl"));
```

在 `create_bundle_en_us` 函数中，`add_resource` 调用列表末尾新增：

```rust
add_resource(&mut bundle, include_str!("../locales/en-US/file-tree.ftl"));
```

- [ ] **步骤 6：运行 i18n 测试验证**

```bash
cargo test -p i18n
```

预期：全部通过，新 key 在编译期解析成功。

- [ ] **步骤 7：Commit**

```bash
git add crates/i18n/
git commit -m "feat(i18n): add file tree panel translation keys"
```

---

### 任务 3：实现 FileTreeState 数据结构与核心逻辑

**文件：**
- 创建：`crates/zdown-app/src/file_tree.rs`

- [ ] **步骤 1：编写失败的单元测试**

在 `crates/zdown-app/src/file_tree.rs` 创建，先写测试：

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("create temp dir");
        let root = dir.path().to_path_buf();
        // 创建目录结构:
        // root/
        //   README.md
        //   docs/
        //     guide.md
        //   src/
        //     main.rs
        //   .hidden/
        //     secret.md
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
        // 应该有根节点 + 3 个可见子节点（docs, src, README.md）
        // + docs 下有 guide.md + src 下有 main.rs
        // 默认只展开根目录
        assert!(state.nodes.len() >= 4, "expected at least root + 3 visible children");
        // README.md 应该存在
        let readme = state.nodes.iter().find(|n| n.name == "README.md");
        assert!(readme.is_some(), "README.md should exist");
        assert!(!readme.unwrap().is_dir);
        // .hidden 不应出现
        let hidden = state.nodes.iter().find(|n| n.name == ".hidden");
        assert!(hidden.is_none(), ".hidden should be skipped");
    }

    #[test]
    fn toggle_expand_directory() {
        let (_dir, root) = setup_test_dir();
        let mut state = FileTreeState::default();
        state.open_folder(&root);

        // 找到 docs 目录节点
        let docs_idx = state
            .nodes
            .iter()
            .position(|n| n.name == "docs" && n.is_dir)
            .expect("docs dir should exist");

        // 初始未展开
        assert!(!state.is_expanded(&state.nodes[docs_idx].path));

        // 展开
        state.toggle_expand(docs_idx);
        assert!(state.is_expanded(&state.nodes[docs_idx].path));
        // 展开后 guide.md 应出现
        let guide = state.nodes.iter().find(|n| n.name == "guide.md");
        assert!(guide.is_some(), "guide.md should appear after expand");

        // 折叠
        state.toggle_expand(docs_idx);
        assert!(!state.is_expanded(&state.nodes[docs_idx].path));
    }

    #[test]
    fn empty_folder_has_only_root() {
        let dir = TempDir::new().expect("create temp dir");
        let mut state = FileTreeState::default();
        state.open_folder(dir.path());
        // 空目录只有根节点
        assert_eq!(state.nodes.len(), 1);
        assert_eq!(state.nodes[0].depth, 0);
    }

    #[test]
    fn nodes_sorted_dirs_first() {
        let (_dir, root) = setup_test_dir();
        let mut state = FileTreeState::default();
        state.open_folder(&root);

        // 根的直接子节点：目录在前，文件在后
        let root_children: Vec<&FileTreeNode> = state
            .nodes
            .iter()
            .filter(|n| n.depth == 1)
            .collect();

        // 前面应该是目录
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
```

- [ ] **步骤 2：运行测试确认失败**

```bash
cargo test -p zdown-app -- file_tree
```

预期：编译错误（`FileTreeState` 不存在）

- [ ] **步骤 3：在 `main.rs` 中声明模块**

在 `crates/zdown-app/src/main.rs` 的 mod 声明区域（约第 16 行）新增：

```rust
mod file_tree;
```

- [ ] **步骤 4：实现 FileTreeNode 和 FileTreeState**

在 `crates/zdown-app/src/file_tree.rs` 顶部编写实现（测试模块之前）：

```rust
//! 文件树面板：浏览文件夹目录结构，打开 Markdown 文件。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 文件树中的一个节点。
///
/// 节点扁平化存储在 `FileTreeState::nodes` 中。
/// 目录有两种状态：折叠（子节点不在 nodes 中）和展开（子节点已加载到 nodes）。
#[derive(Debug, Clone)]
pub struct FileTreeNode {
    /// 显示名称（文件名或目录名）。
    pub name: String,
    /// 文件系统绝对路径。
    pub path: PathBuf,
    /// 是否为目录。
    pub is_dir: bool,
    /// 缩进层级（0 = 根目录）。
    pub depth: usize,
}

/// 文件树面板的完整状态。
#[derive(Debug, Clone, Default)]
pub struct FileTreeState {
    /// 当前打开的文件夹路径。None 表示未打开文件夹。
    pub root_path: Option<PathBuf>,
    /// 当前可见的节点列表（只包含已展开目录的子节点）。
    pub nodes: Vec<FileTreeNode>,
    /// 已展开的目录路径集合。
    expanded: BTreeSet<PathBuf>,
    /// 右键菜单状态。
    pub context_menu: Option<FileContextMenu>,
    /// 内联重命名状态：(节点索引, 编辑中的新名称)。
    pub renaming: Option<(usize, String)>,
    /// 新建文件输入状态：(父目录路径, 编辑中的文件名)。
    pub new_file_input: Option<(PathBuf, String, bool)>, // (parent_path, name, focus)
}

impl FileTreeState {
    /// 打开文件夹并扫描根目录的直接子项。
    pub fn open_folder(&mut self, path: &Path) {
        self.root_path = Some(path.to_path_buf());
        self.nodes.clear();
        self.expanded.clear();
        self.context_menu = None;
        self.renaming = None;
        self.new_file_input = None;

        // 添加根节点
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

        // 默认展开根目录
        self.expanded.insert(path.to_path_buf());
        self.load_children(0);
    }

    /// 加载指定目录节点的直接子项到 nodes 列表中。
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
            // 跳过隐藏文件/目录
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

        // 排序：目录优先，然后按名称字母序
        children.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        // 拼接节点：找到父节点后可插入的起始位置
        // 插入位置为 parent_idx 之后、下一个同深度或更浅节点的之前
        let insert_pos = self.find_insert_pos(parent_idx);

        let count = children.len();
        // 将后续节点的深度引用偏移 count
        self.nodes.splice(insert_pos..insert_pos, children);
    }

    /// 找到 parent 的子节点插入位置：所有 parent 之后的节点中，
    /// 第一个 depth <= parent.depth 的节点位置之后（即 parent 的"范围"结束位置）。
    fn find_insert_pos(&self, parent_idx: usize) -> usize {
        let parent_depth = self.nodes[parent_idx].depth;
        let mut pos = parent_idx + 1;
        while pos < self.nodes.len() && self.nodes[pos].depth > parent_depth {
            pos += 1;
        }
        pos
    }

    /// 折叠/展开目录节点。展开时按需加载子项。
    pub fn toggle_expand(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];
        if !node.is_dir {
            return;
        }

        if self.expanded.contains(&node.path) {
            // 折叠：移除所有后代节点
            self.expanded.remove(&node.path);
            self.remove_descendants(node_idx);
        } else {
            // 展开
            self.expanded.insert(node.path.clone());
            self.load_children(node_idx);
        }
    }

    /// 移除指定节点的所有后代。
    fn remove_descendants(&mut self, parent_idx: usize) {
        let parent_depth = self.nodes[parent_idx].depth;
        let mut remove_start = parent_idx + 1;
        while remove_start < self.nodes.len()
            && self.nodes[remove_start].depth > parent_depth
        {
            remove_start += 1;
        }
        let remove_count = remove_start - parent_idx - 1;
        if remove_count > 0 {
            // 同时清理被移除的展开状态
            for i in parent_idx + 1..remove_start {
                self.expanded.remove(&self.nodes[i].path);
            }
            self.nodes.drain(parent_idx + 1..remove_start);
        }
    }

    /// 检查路径对应的目录是否已展开。
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// 按路径查找节点索引。
    pub fn find_by_path(&self, path: &Path) -> Option<usize> {
        self.nodes.iter().position(|n| n.path == path)
    }

    /// 重新扫描根目录。
    pub fn refresh(&mut self) {
        if let Some(root) = self.root_path.clone() {
            self.open_folder(&root);
        }
    }

    /// 递归展开指定目录节点的所有子目录。
    pub fn expand_all(&mut self, node_idx: usize) {
        let node = &self.nodes[node_idx];
        if !node.is_dir {
            return;
        }
        if !self.expanded.contains(&node.path) {
            self.expanded.insert(node.path.clone());
            self.load_children(node_idx);
        }
        // 递归展开子目录 —— 由于 load_children 会修改 nodes，
        // 先收集所有子目录的路径
        let child_paths: Vec<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| n.depth > self.nodes[node_idx].depth && n.is_dir)
            .map(|n| n.path.clone())
            .collect();

        for path in child_paths {
            if let Some(idx) = self.find_by_path(&path) {
                self.expand_all(idx);
            }
        }
    }

    /// 递归折叠指定目录节点的所有子目录。
    pub fn collapse_all(&mut self, node_idx: usize) {
        let node_path = self.nodes[node_idx].path.clone();
        if !self.nodes[node_idx].is_dir {
            return;
        }
        // 收集所有后代目录的路径
        let descendant_dirs: Vec<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| {
                n.is_dir
                    && n.depth > self.nodes[node_idx].depth
                    && n.path.starts_with(&node_path)
            })
            .map(|n| n.path.clone())
            .collect();

        for path in &descendant_dirs {
            self.expanded.remove(path);
        }
        // 折叠当前节点
        self.expanded.remove(&node_path);
        self.remove_descendants(node_idx);
    }

    /// 判断给定路径是否为 Markdown 文件。
    pub fn is_markdown(path: &Path) -> bool {
        path.extension()
            .map(|e| {
                let e = e.to_ascii_lowercase();
                let e_str = e.to_string_lossy();
                e_str == "md" || e_str == "markdown"
            })
            .unwrap_or(false)
    }
}

/// 右键菜单状态。
#[derive(Debug, Clone)]
pub struct FileContextMenu {
    pub node_index: usize,
    /// 菜单锚点（屏幕坐标）。
    pub anchor: egui::Pos2,
}
```

- [ ] **步骤 5：运行测试验证通过**

```bash
cargo test -p zdown-app -- file_tree
```

预期：全部 PASS

- [ ] **步骤 6：Commit**

```bash
git add crates/zdown-app/src/file_tree.rs crates/zdown-app/src/main.rs
git commit -m "feat(file-tree): add FileTreeState data structure and core logic"
```

---

### 任务 4：实现文件树 UI 渲染

**文件：**
- 修改：`crates/zdown-app/src/file_tree.rs`（追加 UI 函数）

- [ ] **步骤 1：实现 `show_file_tree_panel` 入口函数**

在 `file_tree.rs` 的 `FileTreeState` impl 块之后、测试模块之前追加：

```rust
/// 渲染文件树面板。
///
/// 在大纲面板下方的同一 SidePanel 内调用。
pub fn show_file_tree_panel(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    editor_state: &mut crate::editor_state::EditorState,
    i18n: &crate::i18n::I18n,
) {
    // 面板标题
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("📁").size(14.0));
        if let Some(ref root) = state.root_path {
            ui.label(
                egui::RichText::new(root.display().to_string())
                    .size(13.0)
                    .strong(),
            );
        } else {
            ui.label(egui::RichText::new(i18n.t("file-tree-empty-hint")).size(13.0).weak());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.root_path.is_some() {
                if ui
                    .add(egui::Button::new("🔄").min_size(egui::vec2(20.0, 16.0)))
                    .on_hover_text(i18n.t("file-tree-refresh"))
                    .clicked()
                {
                    state.refresh();
                }
            }
        });
    });

    ui.separator();

    // 节点列表
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
            // 收集所有可见节点的索引（避免迭代中修改 borrow）
            let node_count = state.nodes.len();
            let mut action: Option<FileTreeAction> = None;
            let mut ctx_menu: Option<FileContextMenu> = None;

            for i in 0..node_count {
                let resp = render_node(ui, state, i, i18n);
                if resp.clicked() {
                    action = Some(handle_node_click(state, i, editor_state));
                }
                if resp.secondary_clicked() {
                    ctx_menu = Some(FileContextMenu {
                        node_index: i,
                        anchor: resp.rect.left_bottom(),
                    });
                }
            }

            // 处理单击动作
            if let Some(act) = action {
                match act {
                    FileTreeAction::OpenFile(path) => {
                        let _ = editor_state.open(&path);
                    }
                }
            }

            // 设置右键菜单
            state.context_menu = ctx_menu;
        });

    // 渲染右键菜单
    if let Some(ref menu) = state.context_menu.clone() {
        show_context_menu(ui, state, menu, editor_state, i18n);
    }

    // 渲染内联重命名
    if let Some((idx, ref mut new_name)) = state.renaming {
        show_inline_rename(ui, state, idx, new_name, i18n);
    }

    // 渲染新建文件输入
    if let Some((ref parent, ref mut name, ref mut focus)) = state.new_file_input.clone() {
        show_new_file_input(ui, state, &parent, name, focus, editor_state, i18n);
    }
}

enum FileTreeAction {
    OpenFile(PathBuf),
}

/// 渲染单个节点。
fn render_node(
    ui: &mut egui::Ui,
    state: &FileTreeState,
    idx: usize,
    i18n: &crate::i18n::I18n,
) -> egui::Response {
    let node = &state.nodes[idx];
    let indent = node.depth as f32 * 16.0;

    ui.horizontal(|ui| {
        ui.add_space(indent);

        // 折叠/展开三角（仅目录）
        if node.is_dir {
            let expanded = state.is_expanded(&node.path);
            let arrow = if expanded { "▼" } else { "▶" };
            let arrow_resp = ui.add(
                egui::SelectableLabel::new(false, egui::RichText::new(arrow).size(11.0))
            );
            if arrow_resp.clicked() {
                // 将在调用方处理 toggle
            }
        } else {
            ui.add_space(16.0); // 对齐三角位置
        }

        // 图标 + 名称
        let icon = if node.is_dir {
            if state.is_expanded(&node.path) { "📂" } else { "📁" }
        } else if FileTreeState::is_markdown(&node.path) {
            "📝"
        } else {
            "📄"
        };

        let name_color = if FileTreeState::is_markdown(&node.path) {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_gray(160)
        };

        let name_text = egui::RichText::new(format!("{icon} {}", node.name))
            .color(name_color)
            .size(13.0);

        let resp = ui.add(
            egui::SelectableLabel::new(false, name_text)
        );

        // Tooltip 显示完整路径
        if FileTreeState::is_markdown(&node.path) {
            resp.clone().on_hover_text(node.path.display().to_string())
        } else {
            resp
        }
    })
    .response
}

/// 处理节点点击。返回需要执行的动作。
fn handle_node_click(
    state: &mut FileTreeState,
    idx: usize,
    editor_state: &mut crate::editor_state::EditorState,
) -> FileTreeAction {
    let node = &state.nodes[idx];
    if node.is_dir {
        state.toggle_expand(idx);
        return FileTreeAction::OpenFile(PathBuf::new()); // dummy
    }

    if FileTreeState::is_markdown(&node.path) {
        return FileTreeAction::OpenFile(node.path.clone());
    }

    // 非 Markdown 文件：显示提示
    editor_state.status_message = editor_state.i18n_t_placeholder("file-tree-unsupported");
    FileTreeAction::OpenFile(PathBuf::new()) // dummy
}
```

- [ ] **步骤 2：等待任务 5 完成右键菜单后再回来验证编译**

- [ ] **步骤 3：Commit**（与任务 5 一起）

---

### 任务 5：实现右键菜单

**文件：**
- 修改：`crates/zdown-app/src/file_tree.rs`（追加右键菜单 UI）

- [ ] **步骤 1：实现 `show_context_menu` 函数**

在 `file_tree.rs` 的 `handle_node_click` 之后追加：

```rust
/// 渲染右键菜单浮层。
fn show_context_menu(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    menu: &FileContextMenu,
    editor_state: &mut crate::editor_state::EditorState,
    i18n: &crate::i18n::I18n,
) {
    let node = &state.nodes[menu.node_index];
    let egui_ctx = ui.ctx().clone();

    egui::Area::new("file_tree_context_menu".into())
        .fixed_pos(menu.anchor)
        .order(egui::Order::Foreground)
        .show(&egui_ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);

                if node.is_dir {
                    if ui.button(format!("📂 {}", i18n.t("file-tree-expand-all"))).clicked() {
                        state.expand_all(menu.node_index);
                        state.context_menu = None;
                    }
                    if ui.button(format!("📁 {}", i18n.t("file-tree-collapse-all"))).clicked() {
                        state.collapse_all(menu.node_index);
                        state.context_menu = None;
                    }
                    ui.separator();
                    if ui.button(format!("➕ {}", i18n.t("file-tree-new-file"))).clicked() {
                        state.new_file_input = Some((
                            node.path.clone(),
                            String::new(),
                            true,
                        ));
                        state.context_menu = None;
                    }
                } else {
                    if ui.button(format!("📄 {}", i18n.t("file-tree-open"))).clicked() {
                        let path = node.path.clone();
                        let _ = editor_state.open(&path);
                        state.context_menu = None;
                    }
                }

                ui.separator();

                if ui.button(format!("✏️ {}", i18n.t("file-tree-rename"))).clicked() {
                    state.renaming = Some((menu.node_index, node.name.clone()));
                    state.context_menu = None;
                }

                if ui.button(format!("🗑️ {}", i18n.t("file-tree-delete"))).clicked() {
                    let path = node.path.clone();
                    let name = node.name.clone();
                    let is_dir = node.is_dir;
                    // 执行删除
                    match if is_dir {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    } {
                        Ok(()) => {
                            state.refresh();
                            // 关闭已打开的标签页
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

                if ui.button(format!("📂 {}", i18n.t("file-tree-reveal"))).clicked() {
                    let reveal_path = if node.is_dir {
                        node.path.clone()
                    } else {
                        node.path.parent().map(|p| p.to_path_buf()).unwrap_or_default()
                    };
                    let _ = open::that(&reveal_path);
                    state.context_menu = None;
                }
            });
        });

    // 点击菜单外关闭
    if egui_ctx.input(|i| i.key_pressed(egui::Key::Escape))
        || egui_ctx.input(|i| i.pointer.primary_clicked())
    {
        state.context_menu = None;
    }
}

/// 渲染内联重命名输入框。
fn show_inline_rename(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    idx: usize,
    new_name: &mut String,
    i18n: &crate::i18n::I18n,
) {
    let egui_ctx = ui.ctx().clone();

    egui::Window::new(i18n.t("file-tree-rename"))
        .collapsible(false)
        .resizable(false)
        .min_size(egui::vec2(300.0, 80.0))
        .show(&egui_ctx, |ui| {
            ui.label(i18n.t("file-tree-rename-prompt"));
            let resp = ui.add(
                egui::TextEdit::singleline(new_name)
                    .desired_width(260.0),
            );

            ui.horizontal(|ui| {
                if ui.button("✓").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let old_path = state.nodes[idx].path.clone();
                    let parent = old_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                    let new_path = parent.join(&*new_name);

                    if !new_path.exists() {
                        let _ = std::fs::rename(&old_path, &new_path);
                        state.refresh();
                    }
                    state.renaming = None;
                }
                if ui.button("✗").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    state.renaming = None;
                }
            });

            if resp.changed() {
                // 触发重绘
            }
        });
}

/// 渲染新建文件输入框。
fn show_new_file_input(
    ui: &mut egui::Ui,
    state: &mut FileTreeState,
    parent: &Path,
    name: &mut String,
    focus: &mut bool,
    editor_state: &mut crate::editor_state::EditorState,
    i18n: &crate::i18n::I18n,
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

            let resp = ui.add(
                egui::TextEdit::singleline(name)
                    .id(id)
                    .hint_text("filename.md")
                    .desired_width(260.0),
            );

            ui.horizontal(|ui| {
                if ui.button("✓").clicked() || (ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty()) {
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
                            editor_state.status_message =
                                format!("Failed to create file: {e}");
                        }
                    }
                    state.new_file_input = None;
                }
                if ui.button("✗").clicked() {
                    state.new_file_input = None;
                }
            });
        });
}
```

- [ ] **步骤 2：给 `EditorState` 添加所需方法**

修改 `crates/zdown-app/src/editor_state.rs`，在 impl 块中新增：

```rust
/// 为 file_tree 模块提供的 i18n 占位方法。
/// i18n 由调用方持有，这里用硬编码回退。
pub fn i18n_t_placeholder(&self, key: &str) -> String {
    key.to_string()
}

/// 按路径关闭对应的标签页。若路径匹配当前标签页，关闭后切换到相邻标签页。
/// 返回是否执行了关闭操作。
pub fn close_tab_by_path(&mut self, path: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(idx) = self.tabs.iter().position(|t| {
        t.path
            .as_ref()
            .and_then(|p| p.canonicalize().ok())
            .map_or(false, |p| p == canonical)
    }) {
        return self.close_tab(idx);
    }
    false
}
```

- [ ] **步骤 3：运行编译和测试验证**

```bash
cargo check -p zdown-app
cargo test -p zdown-app -- file_tree
```

预期：编译通过，测试 PASS

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/file_tree.rs crates/zdown-app/src/editor_state.rs
git commit -m "feat(file-tree): implement context menu, inline rename, and new file dialogs"
```

---

### 任务 6：集成到 ZdownApp 布局 + 菜单入口

**文件：**
- 修改：`crates/zdown-app/src/main.rs:138-166`（ZdownApp struct + default）
- 修改：`crates/zdown-app/src/main.rs:335-349`（SidePanel 区域）
- 修改：`crates/zdown-app/src/menu.rs:50-130`（菜单项）
- 修改：`crates/zdown-app/src/menu.rs:247-252`（新增 trigger_open_folder）

- [ ] **步骤 1：在 ZdownApp struct 中添加 file_tree 字段**

修改 `crates/zdown-app/src/main.rs`，在 `ZdownApp` struct 的 `terminal` 字段之后：

```rust
/// 文件树面板。
file_tree: file_tree::FileTreeState,
```

- [ ] **步骤 2：在 Default 中初始化**

在 `Default for ZdownApp` 方法末尾（`terminal: TerminalPanel::default()` 之后）：

```rust
file_tree: file_tree::FileTreeState::default(),
```

- [ ] **步骤 3：在大纲面板下添加文件树面板**

修改 `crates/zdown-app/src/main.rs` 的 SidePanel 区域（当前第 335-349 行），在大纲面板内部末尾和 `SidePanel` 闭合之前，插入文件树渲染：

需要把 SidePanel 从只包含大纲改为包含大纲 + 分隔线 + 文件树。修改方案：

```rust
egui::SidePanel::left("outline_panel")
    .resizable(true)
    .default_width(200.0)
    .min_width(60.0)
    .show_inside(ui, |ui| {
        // 上半部：大纲面板
        egui::TopBottomPanel::top("outline_section")
            .resizable(true)
            .min_height(60.0)
            .default_height(ui.available_height() * 0.5)
            .show_inside(ui, |ui| {
                outline_view::show_outline_panel(
                    ui,
                    &mut self.state,
                    &mut self.fold_state,
                    &mut self.outline_drag,
                    &mut self.outline_filter,
                    &self.i18n,
                );
            });

        // 下半部：文件树面板
        egui::TopBottomPanel::bottom("file_tree_section")
            .resizable(true)
            .min_height(40.0)
            .show_inside(ui, |ui| {
                file_tree::show_file_tree_panel(
                    ui,
                    &mut self.file_tree,
                    &mut self.state,
                    &self.i18n,
                );
            });
    });
```

- [ ] **步骤 4：添加拖拽文件夹的处理**

在 `ui` 方法中，菜单处理之后、`show_confirm_dialog` 之前，新增拖拽检测：

```rust
// 拖拽文件夹到窗口：打开文件夹
ctx.input(|i| {
    for file in &i.raw.dropped_files {
        if let Some(path) = &file.path {
            if path.is_dir() {
                self.file_tree.open_folder(path);
            }
        }
    }
});
```

- [ ] **步骤 5：在菜单中添加"打开文件夹"项**

修改 `crates/zdown-app/src/menu.rs` 的 `show_menu` 函数，在 `menu-file-open` 按钮之后新增：

```rust
if ui.button(i18n.t("menu-file-open-folder")).clicked() {
    trigger_open_folder(state, file_tree, i18n);
}
```

需修改 `show_menu` 函数签名，新增 `file_tree: &mut FileTreeState` 参数：

```rust
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    settings_dialog: &mut SettingsDialog,
    app_config: &AppConfig,
    theme: &mut ThemeMode,
    _image_hosting: &ImageHostingConfig,
    i18n: &I18n,
    terminal: &mut TerminalPanel,
    file_tree: &mut file_tree::FileTreeState,  // 新增
) {
```

同时修改 `main.rs` 中调用 `show_menu` 处，传入 `&mut self.file_tree`。

- [ ] **步骤 6：实现 `trigger_open_folder` 函数**

在 `menu.rs` 的 `trigger_open` 之后新增：

```rust
fn trigger_open_folder(
    _state: &EditorState,
    file_tree: &mut crate::file_tree::FileTreeState,
    i18n: &I18n,
) {
    let title = i18n.t("menu-file-open-folder");
    if let Some(path) = workspace::pick_folder(&title) {
        file_tree.open_folder(&path);
    }
}
```

- [ ] **步骤 7：运行编译和整体测试**

```bash
cargo check
cargo test
cargo fmt
cargo clippy -- -D warnings
```

预期：全部通过

- [ ] **步骤 8：Commit**

```bash
git add -A
git commit -m "feat(file-tree): integrate file tree panel into ZdownApp layout and menu"
```

---

### 任务 7：修复 status_message 在 file_tree 中的 i18n 依赖

**文件：**
- 修改：`crates/zdown-app/src/file_tree.rs`

- [ ] **步骤 1：移除 `i18n_t_placeholder`，改为直接设置英文回退**

修改 `handle_node_click` 函数中设置 status_message 的行：

```rust
// 原来:
editor_state.status_message = editor_state.i18n_t_placeholder("file-tree-unsupported");

// 改为:
editor_state.status_message = "This file type is not supported".to_string();
```

- [ ] **步骤 2：移除 EditorState 中不再需要的 `i18n_t_placeholder` 方法**

从 `crates/zdown-app/src/editor_state.rs` 中删除之前添加的 `i18n_t_placeholder` 方法。

- [ ] **步骤 3：为 i18n 化的右键菜单消息提供更友好的处理**

将右键菜单中的 status_message 也使用简洁的英文回退（已在实现中）。

- [ ] **步骤 4：运行测试和 clippy**

```bash
cargo test
cargo clippy -- -D warnings
```

- [ ] **步骤 5：Commit**

```bash
git add -A
git commit -m "fix(file-tree): use direct english fallback for status messages"
```

---

## 自检

### 1. 规格覆盖度

| 规格章节 | 对应任务 |
|----------|----------|
| §3 数据结构（FileTreeNode, FileTreeState, FileContextMenu） | 任务 3 |
| §4 UI 布局（大纲在上、文件树在下、可拖拽分隔） | 任务 6 步骤 3 |
| §4.3 未打开文件夹提示 | 任务 4 步骤 1 |
| §4.4 节点渲染（缩进、三角、图标、高亮、tooltip） | 任务 4 步骤 1 |
| §5.1 单击行为（展开目录、打开 .md、跳过非 .md） | 任务 4 步骤 1 |
| §5.3 右键菜单（文件节点 + 目录节点） | 任务 5 步骤 1 |
| §5.4 内联重命名 | 任务 5 步骤 1 |
| §5.5 新建 Markdown 文件 | 任务 5 步骤 1 |
| §6 文件操作 API（按需展开、排序） | 任务 3 步骤 4 |
| §7 拖拽支持 | 任务 6 步骤 4 |
| §8 国际化键 | 任务 2 |
| §9 错误处理 | 任务 3-5（tracing::warn + status_message） |
| §10 测试 | 任务 3 步骤 1、任务 6 步骤 7 |

### 2. 占位符扫描

- 无 TODO/待定 ✓
- 所有步骤都有完整代码或明确命令 ✓
- 所有类型、函数名在任务间一致 ✓

### 3. 类型一致性

- `FileTreeState` 在任务 3 定义，任务 4-6 使用 ✓
- `FileTreeNode` 字段名 `path`, `is_dir`, `depth`, `name` 一致 ✓
- `FileContextMenu` 字段 `node_index`, `anchor` 一致 ✓
- `show_file_tree_panel` 签名在任务 4 和任务 6 调用处匹配 ✓
- `pick_folder` 在任务 1 定义，任务 6 使用 ✓
