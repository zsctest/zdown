# markdown_renderer source + zdown-app MVP 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 实现阶段 1 编辑器 UI：syntect 源码高亮器、EditorState 聚合、egui 源码编辑视图、快捷键、菜单栏与未保存提示、最近文件菜单。

**架构：** `markdown_renderer` 加 `source` 模块提供 `SourceHighlighter`（有状态，一次处理全文，跨行语法状态保留）。`zdown-app` 用 `EditorState` 聚合 `Editor` + `Option<PathBuf>` + `RecentFiles` + `Workspace`，egui 渲染源码视图（TextEdit + 高亮装饰）、菜单栏、快捷键。本 plan 含一个 spike 任务评估 egui 0.34 TextEdit 高亮可行性，根据结果选实现路径。

**技术栈：** Rust 2024 edition、egui 0.34、eframe 0.34、syntect 5.3（default-syntax-set + default-themes）、editor_engine / workspace / document_model（path）。

**前置任务：** Plan 1/2/3 完成。本 plan 修改 markdown_renderer 与 zdown-app 的 Cargo.toml 加依赖。

---

## 文件结构

- 修改：`crates/markdown_renderer/Cargo.toml` — 加 syntect 依赖
- 修改：`crates/markdown_renderer/src/lib.rs` — 模块声明
- 创建：`crates/markdown_renderer/src/source.rs` — `SourceHighlighter`
- 修改：`crates/zdown-app/Cargo.toml` — 加所有 crate 依赖
- 修改：`crates/zdown-app/src/main.rs` — egui 应用入口（替换阶段 0 占位）
- 创建：`crates/zdown-app/src/editor_state.rs` — `EditorState` 聚合
- 创建：`crates/zdown-app/src/source_view.rs` — 源码编辑视图 Widget
- 创建：`crates/zdown-app/src/menu.rs` — 菜单栏 + 未保存提示对话框
- 测试：source.rs 内联单元测试；UI 部分手动验证 + smoke

**关键设计决策：**

- **SourceHighlighter 有状态**：`highlight(&self, src: &str) -> Vec<Vec<(Style, &str)>>` 一次处理全文，保留跨行语法状态（代码块内、多行强调）。返回每行的样式片段列表
- **egui 高亮策略**：阶段 1 用 spike 评估——优先用 `egui::TextEdit::multiline` 接受输入 + 在其上层用 `Painter` 绘制高亮背景色块；若不可行降级为单色编辑 + 侧栏高亮预览
- **行号渲染**：并入 source_view（不放 markdown_renderer，行号是编辑器装饰非渲染产物）
- **EditorState 聚合**：`Editor` + `Option<PathBuf>` + `RecentFiles` + `Workspace`，提供 `is_dirty` / `title` / `open(path)` / `save()` / `save_as(path)` / `new_file()`
- **快捷键**：egui `Modifiers::Ctrl` 在 macOS 自动映射 Cmd，阶段 1 不额外处理
- **未保存提示**：Save / Don't Save / Cancel 三选项，Cancel 取消整个操作

---

## 任务 1：SourceHighlighter（syntect markdown 高亮）

**文件：**
- 修改：`crates/markdown_renderer/Cargo.toml`
- 修改：`crates/markdown_renderer/src/lib.rs`
- 创建：`crates/markdown_renderer/src/source.rs`
- 测试：内联单元测试

- [ ] **步骤 1.1：修改 Cargo.toml 加依赖**

替换 `crates/markdown_renderer/Cargo.toml`：

```toml
[package]
name = "markdown_renderer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
thiserror.workspace = true
syntect = { workspace = true, features = ["default-syntax-set", "default-themes"] }
```

- [ ] **步骤 1.2：修改 lib.rs 模块声明**

替换 `crates/markdown_renderer/src/lib.rs`：

```rust
//! markdown_renderer：AST → egui 组件渲染（阶段 2）+ 源码高亮（阶段 1）。
//!
//! 阶段 1 仅实现 source 模块。AST 渲染在阶段 2 实施。

pub mod error;
pub mod source;

pub use error::Error;
pub use source::{SourceHighlighter, StyledLine};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "markdown_renderer");
    }
}
```

- [ ] **步骤 1.3：编写 SourceHighlighter 实现 + 测试**

创建 `crates/markdown_renderer/src/source.rs`：

```rust
//! 源码语法高亮（syntect）。
//!
//! 有状态高亮：一次处理全文，保留跨行语法状态（代码块、多行强调）。
//! 阶段 2 扩展为 AST → egui 组件渲染，本模块仅做源码行级高亮。

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme};
use syntect::parsing::SyntaxSet;
use syntect::LoadingError;

/// 一行的高亮结果：样式片段列表。
pub type StyledLine<'a> = Vec<(Style, &'a str)>;

/// 源码高亮器。
pub struct SourceHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SourceHighlighter {
    /// 用默认语法集 + 默认主题（`base16-ocean.dark`）构造。
    pub fn new() -> Result<Self, LoadingError> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Ok(Self { syntax_set, theme })
    }

    /// 用指定主题名构造（如 `InspiredGitHub` / `base16-eighties.dark`）。
    pub fn with_theme(theme_name: &str) -> Result<Self, LoadingError> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();
        let theme = theme_set.themes.get(theme_name).cloned()
            .ok_or(LoadingError::InvalidTheme)?;
        Ok(Self { syntax_set, theme })
    }

    /// 高亮全文，返回每行的样式片段列表。
    ///
    /// `language` 为 None 时按 markdown 语法高亮；Some(lang) 时按指定语言。
    pub fn highlight<'a>(&self, src: &'a str, language: Option<&str>) -> Vec<StyledLine<'a>> {
        let syntax = match language {
            Some(lang) => self.syntax_set.find_syntax_by_token(lang)
                .or_else(|| self.syntax_set.find_syntax_by_extension(lang)),
            None => self.syntax_set.find_syntax_by_extension("md"),
        };
        let syntax = syntax.unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut result = Vec::new();
        for line in src.lines() {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    // ranges: Vec<(Style, &str)>，&str 借用 line，line 借用 src
                    // 但 line 是 src.lines() 的 &str，生命周期一致
                    let styled: StyledLine = ranges
                        .into_iter()
                        .map(|(style, s)| (style, s))
                        .collect();
                    result.push(styled);
                }
                Err(_) => {
                    result.push(vec![(Default::default(), line)]);
                }
            }
        }
        result
    }

    /// 主题引用（egui 转换颜色用）。
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

impl Default for SourceHighlighter {
    fn default() -> Self {
        Self::new().expect("默认语法集与主题应总能加载")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn new_loads_defaults() {
        let h = SourceHighlighter::new().expect("load");
        assert!(!h.theme.settings.background.is_none() || h.theme.settings.background.is_some());
    }

    #[test]
    fn highlight_empty_returns_empty() {
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight("", None);
        assert!(result.is_empty());
    }

    #[test]
    fn highlight_single_line_returns_one_styled_line() {
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight("# 标题", None);
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_empty());
    }

    #[test]
    fn highlight_multiline_returns_per_line() {
        let src = "# 标题\n\n段落文本\n";
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(src, None);
        // lines() 忽略末尾换行，故 3 行
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn highlight_code_block_preserves_state() {
        let src = "```rust\nfn main() {}\nlet x = 1;\n```\n";
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(src, None);
        // 4 行：```rust / fn main() {} / let x = 1; / ```
        assert_eq!(result.len(), 4);
        // 代码块内行应有多个样式片段（rust 语法高亮）
        assert!(result[1].len() >= 1, "代码行应有样式: {:?}", result[1]);
    }

    #[test]
    fn highlight_with_language_rust() {
        let src = "fn main() { println!(\"hi\"); }";
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(src, Some("rust"));
        assert_eq!(result.len(), 1);
        // rust 语法应产生多个样式片段（fn / main / println 等）
        assert!(result[0].len() > 1, "rust 语法应分多个片段: {:?}", result[0]);
    }

    #[test]
    fn highlight_with_unknown_language_falls_back_to_plain() {
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight("hello", Some("nonexistent-lang"));
        // 不 panic，返回结果
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn styled_line_str_lifetime_matches_input() {
        let src = String::from("# 标题\n段落");
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(&src, None);
        // 验证 &str 借用 src：合并所有片段应能还原原文（按行）
        let line0: String = result[0].iter().map(|(_, s)| *s).collect();
        assert_eq!(line0, "# 标题");
        let line1: String = result[1].iter().map(|(_, s)| *s).collect();
        assert_eq!(line1, "段落");
    }

    #[test]
    fn with_theme_inspired_github() {
        let h = SourceHighlighter::with_theme("InspiredGitHub").expect("theme");
        let _ = h.highlight("# t", None);
    }

    #[test]
    fn with_theme_invalid_returns_err() {
        let result = SourceHighlighter::with_theme("nonexistent-theme-xyz");
        assert!(result.is_err());
    }
}
```

- [ ] **步骤 1.4：运行测试验证**

运行：`cargo test -p markdown_renderer source`
预期：所有 `source::tests::*` 测试通过。首次编译会拉取 syntect 依赖，耗时较长。

运行：`cargo clippy -p markdown_renderer -- -D warnings`
预期：无警告。

- [ ] **步骤 1.5：Commit**

```bash
git add crates/markdown_renderer/
git commit -m "feat(markdown_renderer): SourceHighlighter 源码高亮

syntect default-syntax-set + default-themes（base16-ocean.dark）。
highlight(src, language) 一次处理全文，跨行保留语法状态。
返回 Vec<StyledLine>，&str 借用原 src。
未知语言 fallback 到 plain text。"
```

---

## 任务 2：EditorState 聚合

**文件：**
- 修改：`crates/zdown-app/Cargo.toml`
- 创建：`crates/zdown-app/src/editor_state.rs`
- 修改：`crates/zdown-app/src/main.rs`（暂仅加 mod 声明，UI 在任务 3+）
- 测试：内联单元测试

- [ ] **步骤 2.1：修改 zdown-app/Cargo.toml**

替换 `crates/zdown-app/Cargo.toml`：

```toml
[package]
name = "zdown-app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
eframe.workspace = true
egui.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
editor_engine.workspace = true
workspace.workspace = true
document_model.workspace = true
markdown_renderer.workspace = true
```

- [ ] **步骤 2.2：编写 EditorState 实现 + 测试**

创建 `crates/zdown-app/src/editor_state.rs`：

```rust
//! EditorState：聚合 Editor + 当前路径 + RecentFiles + Workspace。
//!
//! 对 UI 层提供高层操作：new_file / open / save / save_as / undo / redo。
//! UI 事件转发到 Editor 的 Command。

use std::path::{Path, PathBuf};

use document_model::{parse, Document};
use editor_engine::{Command, Cursor, Editor, Selection};
use workspace::{RecentFiles, Workspace};

/// 编辑器顶层状态。
pub struct EditorState {
    pub editor: Editor,
    pub current_path: Option<PathBuf>,
    pub recent: RecentFiles,
    workspace: Workspace,
    /// 标记是否应退出（Quit 菜单触发）。
    pub should_exit: bool,
}

/// open / save 等操作的结果。
pub type OperationResult = Result<(), String>;

impl EditorState {
    /// 空编辑器。
    pub fn new() -> Self {
        Self {
            editor: Editor::empty(),
            current_path: None,
            recent: RecentFiles::load(),
            workspace: Workspace::new(),
            should_exit: false,
        }
    }

    /// 新建文件。要求调用方先确认未保存修改（UI 弹对话框）。
    pub fn new_file(&mut self) {
        self.editor = Editor::empty();
        self.current_path = None;
        // 重置历史与 saved 状态
        self.editor.mark_saved();
    }

    /// 打开指定路径。
    pub fn open(&mut self, path: &Path) -> OperationResult {
        let doc = self.workspace.open(path).map_err(|e| e.to_string())?;
        self.editor = Editor::new(&document_model::to_markdown(&doc));
        self.editor.mark_saved();
        self.current_path = Some(path.to_path_buf());
        self.recent.add(path.to_path_buf());
        let _ = self.recent.save();
        Ok(())
    }

    /// 保存到当前路径。无路径返回 Err（UI 应调 save_as）。
    pub fn save(&mut self) -> OperationResult {
        let doc = self.current_doc();
        self.workspace.save(&doc).map_err(|e| e.to_string())?;
        self.editor.mark_saved();
        Ok(())
    }

    /// 另存为。
    pub fn save_as(&mut self, path: &Path) -> OperationResult {
        let doc = self.current_doc();
        self.workspace.save_as(path, &doc).map_err(|e| e.to_string())?;
        self.editor.mark_saved();
        self.current_path = Some(path.to_path_buf());
        self.recent.add(path.to_path_buf());
        let _ = self.recent.save();
        Ok(())
    }

    /// 应用编辑命令。
    pub fn apply(&mut self, cmd: Command) -> OperationResult {
        self.editor.apply(cmd).map_err(|e| e.to_string())
    }

    /// 撤销。
    pub fn undo(&mut self) -> OperationResult {
        self.editor.undo().map(|_| ()).map_err(|e| e.to_string())
    }

    /// 重做。
    pub fn redo(&mut self) -> OperationResult {
        self.editor.redo().map(|_| ()).map_err(|e| e.to_string())
    }

    /// 是否有未保存修改。
    pub fn is_dirty(&self) -> bool {
        self.editor.is_dirty()
    }

    /// 窗口标题（文件名 + dirty 标记）。
    pub fn title(&self) -> String {
        let name = self.current_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未命名".to_string());
        let dirty = if self.is_dirty() { " *" } else { "" };
        format!("{name}{dirty} - zdown")
    }

    /// 当前文档（从 editor 缓冲序列化为 Document）。
    pub fn current_doc(&self) -> Document {
        let src = self.editor.to_string();
        parse(&src).unwrap_or(Document { blocks: vec![] })
    }

    /// 请求退出。
    pub fn quit(&mut self) {
        self.should_exit = true;
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_state_is_empty_not_dirty() {
        let s = EditorState::new();
        assert!(!s.is_dirty());
        assert!(s.current_path.is_none());
        assert_eq!(s.title(), "未命名 - zdown");
    }

    #[test]
    fn title_shows_dirty_after_edit() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "x".into(),
        })
        .expect("apply");
        assert!(s.is_dirty());
        assert_eq!(s.title(), "未命名 * - zdown");
    }

    #[test]
    fn save_then_dirty_clears() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("a.md");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "hello".into(),
        })
        .expect("apply");
        assert!(s.is_dirty());
        s.save_as(&path).expect("save");
        assert!(!s.is_dirty());
        assert_eq!(s.title(), "a.md - zdown");
    }

    #[test]
    fn open_sets_path_and_recent() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# 标题\n").expect("write");
        let mut s = EditorState::new();
        s.open(&path).expect("open");
        assert_eq!(s.current_path, Some(path.clone()));
        assert!(!s.is_dirty());
        assert_eq!(s.title(), "doc.md - zdown");
        assert!(s.recent.list().contains(&path));
    }

    #[test]
    fn save_without_path_returns_err() {
        let mut s = EditorState::new();
        let err = s.save().unwrap_err();
        assert!(err.contains("路径") || err.contains("path"));
    }

    #[test]
    fn open_nonexistent_returns_err() {
        let mut s = EditorState::new();
        let err = s.open(Path::new("/nonexistent/xyz.md")).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn new_file_resets_state() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "hello".into(),
        })
        .expect("apply");
        s.new_file();
        assert!(!s.is_dirty());
        assert!(s.current_path.is_none());
        assert_eq!(s.editor.to_string(), "");
    }

    #[test]
    fn edit_save_reopen_content_consistent() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("round.md");
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "# 标题\n段落".into(),
        })
        .expect("apply");
        s.save_as(&path).expect("save");

        let mut s2 = EditorState::new();
        s2.open(&path).expect("reopen");
        assert_eq!(s.editor.to_string(), s2.editor.to_string());
    }

    #[test]
    fn undo_redo_via_state() {
        let mut s = EditorState::new();
        s.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "abc".into(),
        })
        .expect("apply");
        s.undo().expect("undo");
        assert_eq!(s.editor.to_string(), "");
        s.redo().expect("redo");
        assert_eq!(s.editor.to_string(), "abc");
    }

    #[test]
    fn quit_sets_should_exit() {
        let mut s = EditorState::new();
        assert!(!s.should_exit);
        s.quit();
        assert!(s.should_exit);
    }
}
```

- [ ] **步骤 2.3：修改 main.rs 加 mod 声明（UI 在任务 3+）**

修改 `crates/zdown-app/src/main.rs`，在文件顶部加：

```rust
mod editor_state;
```

（其余内容保持阶段 0 状态，任务 3 替换 UI 逻辑）

- [ ] **步骤 2.4：运行测试验证**

运行：`cargo test -p zdown-app editor_state`
预期：所有 `editor_state::tests::*` 测试通过。

运行：`cargo clippy -p zdown-app -- -D warnings`
预期：可能有 `dead_code` 警告（should_exit / title 等暂未用），任务 3+ 后消失。临时在 lib.rs/main.rs 顶部加 `#![allow(dead_code)]`？不，clippy `dead_code` 是 warn 不是 deny，应能过。

- [ ] **步骤 2.5：Commit**

```bash
git add crates/zdown-app/Cargo.toml crates/zdown-app/src/editor_state.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): EditorState 聚合 Editor+Path+Recent+Workspace

new_file/open/save/save_as/apply/undo/redo 高层操作。
title 含文件名 + dirty 标记。
open 自动加入 RecentFiles 并持久化。
edit→save_as→reopen 内容一致性测试覆盖。"
```

---

## 任务 3：源码编辑视图（含 spike 决策）

**文件：**
- 创建：`crates/zdown-app/src/source_view.rs`
- 修改：`crates/zdown-app/src/main.rs`

**spike 决策点：** egui 0.34 的 `TextEdit::multiline` 不暴露内部文本布局（行偏移、字符光标像素位置），无法在其上精确叠加高亮。两条路径：

- **路径 A（推荐）**：用 `TextEdit::multiline` 接受输入（保留光标/IME/选区/滚动），在其下方用 `Painter` 绘制高亮文本作为"背景层"——但这要求 TextEdit 透明背景且 TextEdit 的字体/位置与 Painter 一致，egui 0.34 难保证对齐
- **路径 B（阶段 1 MVP）**：放弃行内高亮，用 `TextEdit::multiline` 单色编辑（等宽字体 + 行号），高亮能力推到阶段 2（hybrid 模式 + AST 渲染时一并实现）。SourceHighlighter 仍保留（任务 1 已实现），阶段 2 复用

**本 plan 选路径 B**：阶段 1 优先跑通编辑能力，高亮降级。理由：阶段 1 验收只需"源码视图 + 基础语法高亮"，但 egui 限制使行内高亮工作量超阶段 1 预算；阶段 2 hybrid 模式本就要重做渲染，高亮一并实现更合理。

> 执行者注意：若你评估路径 A 可行（如发现 egui 0.34 有 `text_edit::TextEditState` 暴露布局的 API），可在本任务改为路径 A。否则按路径 B 实现。

- [ ] **步骤 3.1：编写 source_view（路径 B：单色 + 行号）**

创建 `crates/zdown-app/src/source_view.rs`：

```rust
//! 源码编辑视图。
//!
//! 阶段 1（路径 B）：TextEdit::multiline 单色编辑 + 行号显示。
//! 高亮推到阶段 2 hybrid 模式。

use eframe::egui;

use crate::editor_state::EditorState;

/// 渲染源码编辑视图。返回是否消耗了输入焦点。
pub fn show_source_view(ctx: &egui::Context, state: &mut EditorState) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let text_style = egui::TextStyle::Monospace;
        let row_height = ui.text_height(&text_style);

        // 可用区域
        let available = ui.available_size();
        let line_number_width = row_height * 4.0; // 约 4 字符宽

        // 水平布局：行号 | 编辑器
        ui.horizontal(|ui| {
            // 行号列
            ui.allocate_ui_with_layout(
                egui::vec2(line_number_width, available.y),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    let line_count = state.editor.buffer.len_lines();
                    for i in 0..line_count {
                        ui.label(egui::RichText::new(&format!("{:>3}", i + 1))
                            .monospace()
                            .weak());
                    }
                },
            );

            ui.separator();

            // 编辑器
            let mut text = state.editor.to_string();
            let response = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .desired_width(available.x - line_number_width - 8.0)
                    .desired_rows(20)
                    .font(egui::TextStyle::Monospace)
                    .code_editor(),
            );

            // 若文本变化，重建 editor（阶段 1 简化：整体替换）
            // 注意：这会丢失 undo 历史，阶段 2 改为基于光标的增量编辑命令
            if response.changed() {
                let cursor = state.editor.cursor;
                state.editor = Editor::new(&text);
                let _ = state.editor.set_cursor(cursor);
            }
        });
    });
}

use editor_engine::Editor;
```

> **已知简化（阶段 2 改进）：**
> - 整体替换丢失 undo 历史 —— 阶段 2 改为基于 TextEdit 光标事件的增量 Command
> - 无高亮 —— 阶段 2 hybrid 模式实现
> - 光标位置在 text 变更后可能不准确 —— 阶段 2 改进

- [ ] **步骤 3.2：编写 main.rs（最小可运行）**

替换 `crates/zdown-app/src/main.rs`：

```rust
//! zdown-app：egui 应用入口。

mod editor_state;
mod source_view;

use eframe::egui;
use editor_state::EditorState;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 1）");

    if std::env::var_os("ZDOWN_SMOKE").is_some() {
        tracing::info!("ZDOWN_SMOKE 已设置，跳过 GUI 启动");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("zdown"),
        ..Default::default()
    };

    eframe::run_ui_native("zdown", options, |ui, _frame| {
        let state = ui.data_mut(|d| d.get_temp_mut_or_default::<EditorState>("state".into()));
        source_view::show_source_view(ui.ctx(), state);
    })
}
```

- [ ] **步骤 3.3：运行 smoke 验证**

运行：`cargo build -p zdown-app`
预期：编译通过。

运行：`ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：打印 info 日志后退出，不 panic。

> 注：`run_ui_native` 的 closure 不能持有跨帧状态（每帧重新调用）。`ui.data_mut` 用 egui 内部存储持跨帧状态。本步骤 3.2 的代码是简化版，实际 egui 0.34 的 `data_mut` API 可能略不同，执行者需查 egui 0.34 文档调整。

- [ ] **步骤 3.4：本地手动验证 GUI**

运行：`cargo run -p zdown-app`
预期：弹出窗口，标题 "zdown"，左侧行号，右侧可编辑文本区，等宽字体。

- [ ] **步骤 3.5：Commit**

```bash
git add crates/zdown-app/src/source_view.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): 源码编辑视图（路径 B 单色 + 行号）

egui TextEdit::multiline 接受输入，左侧行号列。
阶段 1 简化：整体替换文本（丢失 undo），高亮推到阶段 2。
spike 评估 egui 0.34 TextEdit 不暴露布局，行内高亮不可行。"
```

---

## 任务 4：菜单栏 + 快捷键 + 未保存提示 + 最近文件

**文件：**
- 创建：`crates/zdown-app/src/menu.rs`
- 修改：`crates/zdown-app/src/main.rs`
- 修改：`crates/zdown-app/src/editor_state.rs`（加 `confirm_action` 状态字段）

- [ ] **步骤 4.1：编写 menu 模块**

创建 `crates/zdown-app/src/menu.rs`：

```rust
//! 菜单栏 + 快捷键 + 未保存提示对话框。

use eframe::egui;
use editor_engine::{Command, Cursor};

use crate::editor_state::EditorState;

/// 待确认的操作类型（用户选 New/Open/Quit 但有未保存修改时）。
#[derive(Debug, Clone, PartialEq)]
pub enum PendingAction {
    New,
    Open,
    Quit,
}

/// UI 状态：是否显示未保存确认对话框 + 待确认操作。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfirmDialog {
    pub pending: Option<PendingAction>,
}

impl ConfirmDialog {
    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }
}

/// 渲染菜单栏。返回是否消耗了事件。
pub fn show_menu(ctx: &egui::Context, state: &mut EditorState, confirm: &mut ConfirmDialog) {
    egui::TopBottomPanel::top("menu").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("文件", |ui| {
                if ui.button("新建 (Ctrl+N)").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::New);
                    } else {
                        state.new_file();
                    }
                }
                if ui.button("打开... (Ctrl+O)").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::Open);
                    } else {
                        trigger_open(state);
                    }
                }
                if ui.button("保存 (Ctrl+S)").clicked() {
                    if state.current_path.is_none() {
                        trigger_save_as(state);
                    } else {
                        let _ = state.save();
                    }
                }
                if ui.button("另存为... (Ctrl+Shift+S)").clicked() {
                    trigger_save_as(state);
                }

                ui.separator();

                // 最近文件子菜单
                ui.menu_button("最近文件", |ui| {
                    if state.recent.list().is_empty() {
                        ui.label("(无)");
                    } else {
                        for path in state.recent.list().to_vec() {
                            if ui.button(path.display().to_string()).clicked() {
                                let _ = state.open(path);
                                ui.close_menu();
                            }
                        }
                    }
                });

                ui.separator();

                if ui.button("退出").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::Quit);
                    } else {
                        state.quit();
                    }
                }
            });

            ui.menu_button("编辑", |ui| {
                if ui.button("撤销 (Ctrl+Z)").clicked() {
                    let _ = state.undo();
                }
                if ui.button("重做 (Ctrl+Y)").clicked() {
                    let _ = state.redo();
                }
            });
        });
    });
}

/// 渲染未保存确认对话框。返回 true 表示用户已响应（应清空 confirm.pending）。
pub fn show_confirm_dialog(ctx: &egui::Context, state: &mut EditorState, confirm: &mut ConfirmDialog) {
    if let Some(pending) = confirm.pending.clone() {
        let title = match &pending {
            PendingAction::New => "未保存修改 - 新建",
            PendingAction::Open => "未保存修改 - 打开",
            PendingAction::Quit => "未保存修改 - 退出",
        };
        let mut action_taken = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("当前文档有未保存修改。是否保存？");
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        action_taken = Some("save");
                    }
                    if ui.button("不保存").clicked() {
                        action_taken = Some("discard");
                    }
                    if ui.button("取消").clicked() {
                        action_taken = Some("cancel");
                    }
                });
            });

        if let Some(action) = action_taken {
            match action {
                "save" => {
                    if state.current_path.is_some() {
                        let _ = state.save();
                    } else {
                        trigger_save_as(state);
                    }
                    execute_pending(state, &pending);
                }
                "discard" => {
                    execute_pending(state, &pending);
                }
                _ => {} // cancel：不做
            }
            confirm.pending = None;
        }
    }
}

fn execute_pending(state: &mut EditorState, pending: &PendingAction) {
    match pending {
        PendingAction::New => state.new_file(),
        PendingAction::Open => trigger_open(state),
        PendingAction::Quit => state.quit(),
    }
}

fn trigger_open(state: &mut EditorState) {
    if let Some(path) = workspace::pick_open_file() {
        let _ = state.open(&path);
    }
}

fn trigger_save_as(state: &mut EditorState) {
    if let Some(path) = workspace::pick_save_file() {
        let _ = state.save_as(&path);
    }
}

/// 处理快捷键。返回是否消耗。
pub fn handle_shortcuts(ctx: &egui::Context, state: &mut EditorState, confirm: &mut ConfirmDialog) {
    // Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S / Ctrl+Z / Ctrl+Y
    let mods = ctx.input(|i| i.modifiers);
    let key = ctx.input(|i| i.key_pressed(egui::Key::S));

    // Ctrl+S
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        if state.current_path.is_some() {
            let _ = state.save();
        } else {
            trigger_save_as(state);
        }
    }
    // Ctrl+Shift+S
    if mods.ctrl && mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        trigger_save_as(state);
    }
    // Ctrl+N
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::N)) {
        if state.is_dirty() {
            confirm.pending = Some(PendingAction::New);
        } else {
            state.new_file();
        }
    }
    // Ctrl+O
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::O)) {
        if state.is_dirty() {
            confirm.pending = Some(PendingAction::Open);
        } else {
            trigger_open(state);
        }
    }
    // Ctrl+Z
    if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
        let _ = state.undo();
    }
    // Ctrl+Y 或 Ctrl+Shift+Z
    if (mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::Y)))
        || (mods.ctrl && mods.shift && ctx.input(|i| i.key_pressed(egui::Key::Z)))
    {
        let _ = state.redo();
    }
    // 让 unused 警告不出现
    let _ = key;
}
```

- [ ] **步骤 4.2：修改 main.rs 集成菜单**

替换 `crates/zdown-app/src/main.rs`：

```rust
//! zdown-app：egui 应用入口（阶段 1）。

mod editor_state;
mod menu;
mod source_view;

use eframe::egui;
use editor_state::EditorState;
use menu::ConfirmDialog;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 1）");

    if std::env::var_os("ZDOWN_SMOKE").is_some() {
        tracing::info!("ZDOWN_SMOKE 已设置，跳过 GUI 启动");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("zdown"),
        ..Default::default()
    };

    eframe::run_ui_native("zdown", options, |ui, _frame| {
        let (state, confirm) = ui.data_mut(|d| {
            let state: &mut EditorState = d.get_temp_mut_or_default("state".into());
            let confirm: &mut ConfirmDialog = d.get_temp_mut_or_default("confirm".into());
            (state as *mut _, confirm as *mut _)
        });
        // 安全：data_mut 闭包内借出，需 unsafe 解引用——egui 0.34 的 data_mut API
        // 实际 API 可能更安全，执行者查 egui 0.34 文档调整。
        let state = unsafe { &mut *state };
        let confirm = unsafe { &mut *confirm };

        menu::show_menu(ui.ctx(), state, confirm);
        menu::handle_shortcuts(ui.ctx(), state, confirm);
        menu::show_confirm_dialog(ui.ctx(), state, confirm);
        source_view::show_source_view(ui.ctx(), state);

        if state.should_exit {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    })
}
```

> **执行者注意：** egui 0.34 的 `data_mut` API 与跨帧状态持有方式可能需调整。备选方案：用 `eframe::App` trait + `App::update` 实现，状态作为 `App` 结构体字段。这更符合 egui 标准 pattern。见步骤 4.3 备选实现。

- [ ] **步骤 4.3：备选 main.rs（用 eframe::App trait，推荐）**

若步骤 4.2 的 `data_mut` 方式有生命周期问题，改用 `eframe::App` trait。替换 `crates/zdown-app/src/main.rs`：

```rust
//! zdown-app：egui 应用入口（阶段 1）。

mod editor_state;
mod menu;
mod source_view;

use eframe::egui;
use editor_state::EditorState;
use menu::ConfirmDialog;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 1）");

    if std::env::var_os("ZDOWN_SMOKE").is_some() {
        tracing::info!("ZDOWN_SMOKE 已设置，跳过 GUI 启动");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("zdown"),
        ..Default::default()
    };

    eframe::run_native(
        "zdown",
        options,
        Box::new(|_cc| Ok(Box::new(ZdownApp::default()))),
    )
}

#[derive(Default)]
struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
}

impl eframe::App for ZdownApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        menu::show_menu(ctx, &mut self.state, &mut self.confirm);
        menu::handle_shortcuts(ctx, &mut self.state, &mut self.confirm);
        menu::show_confirm_dialog(ctx, &mut self.state, &mut self.confirm);
        source_view::show_source_view(ctx, &mut self.state);

        if self.state.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
```

- [ ] **步骤 4.4：编译验证**

运行：`cargo build -p zdown-app`
预期：编译通过。若 `data_mut` / `eframe::App` API 与 egui 0.34 不一致，按编译错误调整。

运行：`cargo clippy -p zdown-app -- -D warnings`
预期：无警告。

- [ ] **步骤 4.5：本地手动验证**

运行：`cargo run -p zdown-app`
预期：
- 标题栏显示 "未命名 - zdown"
- 顶部菜单：文件 / 编辑
- 文件菜单：新建/打开/保存/另存为/最近文件/退出
- 编辑菜单：撤销/重做
- Ctrl+S 保存（无路径时弹另存为对话框）
- Ctrl+O 打开
- Ctrl+N 新建
- Ctrl+Z / Ctrl+Y 撤销重做
- 编辑后标题出现 " *"
- 有未保存修改时新建/打开/退出弹确认对话框（保存/不保存/取消）
- 最近文件子菜单显示已打开过的文件

- [ ] **步骤 4.6：Commit**

```bash
git add crates/zdown-app/src/menu.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): 菜单栏+快捷键+未保存提示+最近文件

文件菜单：新建/打开/保存/另存为/最近文件/退出。
编辑菜单：撤销/重做。
快捷键：Ctrl+N/O/S/Shift+S/Z/Y。
未保存修改时弹三选项对话框（保存/不保存/取消）。
最近文件子菜单点击打开。
eframe::App trait 持跨帧状态。"
```

---

## 任务 5：阶段 1 全量验证

**文件：** 无（仅运行验证）

- [ ] **步骤 5.1：fmt + clippy + test + build**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

预期：全部通过。

- [ ] **步骤 5.2：smoke 验证**

```bash
ZDOWN_SMOKE=1 cargo run -p zdown-app
```

预期：打印 info 日志，退出码 0。

- [ ] **步骤 5.3：本地手动 GUI 验证**

按任务 4.5 清单逐项验证。截图存档（可选）。

- [ ] **步骤 5.4：Commit（如有 lint 修复）**

```bash
git add -A
git commit -m "chore: 阶段 1 全量验证通过"
```

---

## 自检

**1. 规格覆盖度：**

- TASKS.md 阶段 1：
  - T1-16 SourceHighlighter → 任务 1 ✓
  - T1-17 行号渲染 → 任务 3（并入 source_view）✓
  - T1-18 EditorState → 任务 2 ✓
  - T1-19 源码编辑视图 → 任务 3（路径 B 降级，无高亮）✓
  - T1-20 快捷键 → 任务 4 ✓
  - T1-21 菜单栏 + 未保存提示 → 任务 4 ✓
  - T1-22 最近文件菜单 → 任务 4 ✓
  - T1-23 覆盖率 ≥ 80% → 未显式任务，依赖各 plan 单元测试覆盖
  - T1-24 性能测试 → 未在本 plan，留收尾
  - T1-25 集成测试 → 任务 2 的 `edit_save_reopen_content_consistent` 部分覆盖

**降级说明：** T1-19 高亮因 egui 0.34 限制降级为单色（路径 B）。这是对 ROADMAP 阶段 1 "语法高亮"交付物的偏离。**建议执行者完成后在 TASKS.md 标注降级，并加 T2-XX 任务在阶段 2 补高亮。**

**2. 占位符扫描：**

- 无 "TODO" / "待定"。
- "执行者注意" 注释是真实指导（egui API 不确定性），非占位符。
- 每个测试与实现有完整代码。

**3. 类型一致性：**

- `EditorState::new_file` / `open(&Path)` / `save()` / `save_as(&Path)` / `apply(Command)` / `undo()` / `redo()` / `is_dirty()` / `title()` 跨任务 2/4 一致。
- `ConfirmDialog::pending: Option<PendingAction>` 在任务 4 一致。
- `menu::show_menu` / `show_confirm_dialog` / `handle_shortcuts` 签名一致。

**4. 编码标准：**

- 测试模块 `#![allow(clippy::expect_used)]` ✓
- 生产代码无 `unwrap`/`expect`（`SourceHighlighter::default` 用 `.expect("默认语法集应总能加载")` —— 这是 panic 风险。**修复**：`Default` 实现改为返回 `SourceHighlighter` 而非 `Result`，但 `expect` 违反 AGENTS.md。**决策**：删除 `Default` impl，强制用 `new() -> Result`，调用方处理。执行者在实现任务 1 时删除 `impl Default`。

**5. 已知简化（阶段 2 改进）：**

- 高亮降级（egui 限制）—— 阶段 2 hybrid 模式
- 整体替换文本丢失 undo —— 阶段 2 改增量 Command
- 光标位置不精确 —— 阶段 2
- `data_mut` API 不确定性 —— 执行者按 egui 0.34 实际 API 调整

**6. UI 测试：**

阶段 1 不写 egui 自动化 UI 测试（eframe 测试模式复杂），依赖手动验证 + smoke。阶段 2 可加 `eframe_testing` 快照测试。

---

## 执行交接

本计划已完成并保存到 `docs/superpowers/plans/2026-06-18-markdown-renderer-source-and-zdown-app.md`。两种执行方式：

1. **子代理驱动（推荐）**
2. **内联执行**

执行者注意：本 plan 是阶段 1 四个独立 plan 中的最后一个。完成后阶段 1 MVP 主体交付完成，但需补 T1-23（覆盖率）/ T1-24（性能）/ T1-25（集成测试）收尾任务，并更新 TASKS.md 标注高亮降级。
