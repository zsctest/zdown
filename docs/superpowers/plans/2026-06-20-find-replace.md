# 查找替换功能 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 源码视图添加查找替换功能，支持 Ctrl+F 搜索栏、实时高亮、匹配导航和替换。

**架构：** 纯逻辑 `SearchEngine`（`search.rs`）负责匹配计算；`SearchState`（`ZdownApp` 持有）管理 UI 状态和匹配结果；搜索栏渲染在 CentralPanel 顶部；匹配高亮在 `source_view.rs` 的 painter 绘制循环中叠加背景色。

**技术栈：** Rust 2024 Edition, egui, ropey（editor_engine 已依赖）, editor_engine::Command

---

### 任务 1：创建 search.rs 模块（纯搜索逻辑 + 单元测试）

**文件：**
- 创建：`crates/zdown-app/src/search.rs`

- [ ] **步骤 1：编写 SearchEngine 和类型定义**

```rust
//! 搜索引擎：纯逻辑模块，不依赖 egui 或 editor_engine。
//!
//! 输入文本字符串和查询，返回匹配位置列表。

/// 搜索选项。
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
}

/// 一个匹配位置。列号为**字符列**（非字节列），与 editor_engine::Cursor 一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// 搜索引擎：零状态，纯函数。
pub struct SearchEngine;

impl SearchEngine {
    /// 在文本中查找所有匹配。
    ///
    /// 逐行扫描。当 `query` 为空时返回空列表。
    pub fn find_all(text: &str, query: &str, opts: &SearchOptions) -> Vec<Match> {
        if query.is_empty() {
            return vec![];
        }
        let mut matches = Vec::new();
        for (line_idx, line) in text.lines().enumerate() {
            find_in_line(&mut matches, line, line_idx, query, opts);
        }
        matches
    }
}

/// 在单行中查找所有匹配。
fn find_in_line(
    matches: &mut Vec<Match>,
    line: &str,
    line_idx: usize,
    query: &str,
    opts: &SearchOptions,
) {
    let line_chars: Vec<char> = line.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();
    let query_len = query_chars.len();

    if line_chars.is_empty() || query_len == 0 || query_len > line_chars.len() {
        return;
    }

    // 准备用于比较的字符切片（区分大小写选项在此处理）
    let cmp_line: Vec<char> = if opts.case_sensitive {
        line_chars.clone()
    } else {
        line_chars.iter().map(|c| c.to_ascii_lowercase()).collect()
    };
    let cmp_query: Vec<char> = if opts.case_sensitive {
        query_chars.clone()
    } else {
        query_chars.iter().map(|c| c.to_ascii_lowercase()).collect()
    };

    let mut col = 0;
    while col + query_len <= cmp_line.len() {
        // 比较字符切片
        if cmp_line[col..col + query_len] == cmp_query[..] {
            let is_match = if opts.whole_word {
                let start_ok =
                    col == 0 || !line_chars[col - 1].is_alphanumeric();
                let end_ok = col + query_len >= line_chars.len()
                    || !line_chars[col + query_len].is_alphanumeric();
                start_ok && end_ok
            } else {
                true
            };
            if is_match {
                matches.push(Match {
                    line: line_idx,
                    col_start: col,
                    col_end: col + query_len,
                });
            }
        }
        col += 1;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn opts_default() -> SearchOptions {
        SearchOptions::default()
    }

    fn opts_case() -> SearchOptions {
        SearchOptions {
            case_sensitive: true,
            whole_word: false,
        }
    }

    fn opts_word() -> SearchOptions {
        SearchOptions {
            case_sensitive: false,
            whole_word: true,
        }
    }

    #[test]
    fn empty_query_returns_empty() {
        let m = SearchEngine::find_all("hello", "", &opts_default());
        assert!(m.is_empty());
    }

    #[test]
    fn empty_text_returns_empty() {
        let m = SearchEngine::find_all("", "hello", &opts_default());
        assert!(m.is_empty());
    }

    #[test]
    fn single_match() {
        let m = SearchEngine::find_all("hello world", "hello", &opts_default());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].line, 0);
        assert_eq!(m[0].col_start, 0);
        assert_eq!(m[0].col_end, 5);
    }

    #[test]
    fn multiple_matches_same_line() {
        let m = SearchEngine::find_all("foo bar foo baz foo", "foo", &opts_default());
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].col_start, 0);
        assert_eq!(m[1].col_start, 8);
        assert_eq!(m[2].col_start, 16);
    }

    #[test]
    fn multiple_lines() {
        let m = SearchEngine::find_all("foo\nbar\nfoo", "foo", &opts_default());
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].line, 0);
        assert_eq!(m[1].line, 2);
    }

    #[test]
    fn case_insensitive_default() {
        let m = SearchEngine::find_all("Hello HELLO hello", "hello", &opts_default());
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn case_sensitive() {
        let m = SearchEngine::find_all("Hello hello HELLO", "hello", &opts_case());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].col_start, 6);
    }

    #[test]
    fn whole_word_basic() {
        let m = SearchEngine::find_all("foo foobar foo", "foo", &opts_word());
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].col_start, 0);
        assert_eq!(m[1].col_start, 11);
    }

    #[test]
    fn whole_word_at_boundaries() {
        let m = SearchEngine::find_all("foo bar-foo foo_bar", "foo", &opts_word());
        // "foo" at start (0), "foo" after hyphen (8), "foo" in "foo_bar" (12) - UNDERSCORE is not alphanumeric? Actually '_' IS alphanumeric in Rust
        // Let's carefully check: is_alphanumeric() for '_' returns false in Rust
        // So "foo_bar": col 12 'f' prev is '_' which is NOT alphanumeric → word start ✓
        //              col 15 end: '_' at col 15 → NOT alphanumeric → word end ✓
        // So "foo" in "foo_bar" IS a whole word match
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn whole_word_with_underscore() {
        // '_' is NOT alphanumeric in Rust, so "foo" in "foo_bar" is a word match
        let m = SearchEngine::find_all("foo_bar foo", "foo", &opts_word());
        // First "foo" at col 0: end char '_' (col 3) is not alphanumeric → match
        // Second "foo" at col 8: start prev ' ' not alphanumeric, end of text → match
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn no_match_returns_empty() {
        let m = SearchEngine::find_all("hello world", "xyz", &opts_default());
        assert!(m.is_empty());
    }

    #[test]
    fn query_longer_than_line() {
        let m = SearchEngine::find_all("hi", "hello", &opts_default());
        assert!(m.is_empty());
    }

    #[test]
    fn unicode_characters() {
        let m = SearchEngine::find_all("你好世界你好", "世界", &opts_default());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].col_start, 2);
        assert_eq!(m[0].col_end, 4);
    }

    #[test]
    fn unicode_case_insensitive() {
        // ASCII lowercase only; non-ASCII chars are preserved as-is
        let m = SearchEngine::find_all("Hello 你好", "hello", &opts_default());
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn adjacent_matches() {
        let m = SearchEngine::find_all("aaa", "aa", &opts_default());
        // Overlapping matches: "aa" at col 0-2, "aa" at col 1-3
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].col_start, 0);
        assert_eq!(m[0].col_end, 2);
        assert_eq!(m[1].col_start, 1);
        assert_eq!(m[1].col_end, 3);
    }
}
```

- [ ] **步骤 2：在 main.rs 中声明 search 模块**

文件：`crates/zdown-app/src/main.rs`

在 `mod outline_view;`（第 7 行）之后添加：

```rust
mod search;
```

- [ ] **步骤 3：运行 search 模块测试验证通过**

运行：
```powershell
cargo test -p zdown-app -- search
```
预期：所有 13 个测试 PASS

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/search.rs crates/zdown-app/src/main.rs
git commit -m "feat(search): add SearchEngine with find_all and unit tests

- SearchEngine: zero-state pure function, char-indexed matching
- SearchOptions: case_sensitive, whole_word toggles
- Match: line + col_start + col_end (char offsets)
- 13 unit tests covering all edge cases

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 2：添加 SearchState + 集成到 ZdownApp

**文件：**
- 创建：`crates/zdown-app/src/search_state.rs`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：创建 SearchState 结构体**

```rust
//! 搜索栏 UI 状态。

use crate::search::{Match, SearchEngine, SearchOptions};

/// 搜索栏 UI 状态，由 ZdownApp 持有。
pub struct SearchState {
    /// 搜索栏是否可见。
    pub visible: bool,
    /// 搜索文本输入。
    pub query: String,
    /// 替换文本输入。
    pub replace: String,
    /// 区分大小写。
    pub case_sensitive: bool,
    /// 全词匹配。
    pub whole_word: bool,
    /// 当前所有匹配位置。
    pub matches: Vec<Match>,
    /// 当前高亮匹配索引。
    pub current_match: Option<usize>,
    /// 下一帧需请求搜索框焦点。
    pub focus_search: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            visible: false,
            query: String::new(),
            replace: String::new(),
            case_sensitive: false,
            whole_word: false,
            matches: Vec::new(),
            current_match: None,
            focus_search: false,
        }
    }
}

impl SearchState {
    /// 用当前查询和选项搜索文本，更新匹配列表。
    pub fn search(&mut self, text: &str) {
        let opts = SearchOptions {
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
        };
        self.matches = SearchEngine::find_all(text, &self.query, &opts);
        if self.matches.is_empty() {
            self.current_match = None;
        } else {
            // 尝试保持当前匹配索引在有效范围
            if let Some(idx) = self.current_match {
                if idx >= self.matches.len() {
                    self.current_match = Some(self.matches.len().saturating_sub(1));
                }
            } else {
                self.current_match = Some(0);
            }
        }
    }

    /// 跳到下一个匹配。返回新匹配位置（用于移动光标）。
    pub fn next_match(&mut self) -> Option<Match> {
        if self.matches.is_empty() {
            return None;
        }
        let next = match self.current_match {
            Some(idx) if idx + 1 < self.matches.len() => idx + 1,
            _ => 0, // 循环回到第一个
        };
        self.current_match = Some(next);
        Some(self.matches[next].clone())
    }

    /// 跳到上一个匹配。
    pub fn prev_match(&mut self) -> Option<Match> {
        if self.matches.is_empty() {
            return None;
        }
        let prev = match self.current_match {
            Some(idx) if idx > 0 => idx - 1,
            _ => self.matches.len().saturating_sub(1), // 循环到最后一个
        };
        self.current_match = Some(prev);
        Some(self.matches[prev].clone())
    }

    /// 关闭搜索栏并清除状态。
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.replace.clear();
        self.matches.clear();
        self.current_match = None;
        self.focus_search = false;
    }

    /// 当前匹配（如果存在）。
    pub fn current_match_pos(&self) -> Option<&Match> {
        self.current_match.and_then(|idx| self.matches.get(idx))
    }
}
```

- [ ] **步骤 2：在 main.rs 中声明模块和导入**

在 `mod search;` 之后添加：

```rust
mod search_state;
```

在 `main.rs` 顶部的 use 区域，添加：

```rust
use search_state::SearchState;
```

- [ ] **步骤 3：在 ZdownApp 中添加 search 字段**

在 `ZdownApp` 结构体的 `settings_dialog` 字段后（第 61 行后）添加：

```rust
    /// 查找替换状态。
    search: SearchState,
```

在 `Default` 实现中（第 75 行后，`settings_dialog` 行后）添加：

```rust
            search: SearchState::default(),
```

- [ ] **步骤 4：编译验证**

运行：
```powershell
cargo check -p zdown-app
```
预期：编译成功，无错误

- [ ] **步骤 5：Commit**

```bash
git add crates/zdown-app/src/search_state.rs crates/zdown-app/src/main.rs
git commit -m "feat(search): add SearchState struct and integrate into ZdownApp

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 3：渲染搜索栏 UI

**文件：**
- 修改：`crates/zdown-app/src/main.rs`（`ui()` 方法中 CentralPanel 部分）

- [ ] **步骤 1：在 CentralPanel 顶部渲染搜索栏**

在 `main.rs` 的 `ui()` 方法中，将第 145-161 行的 `CentralPanel` 部分替换为以下内容。

替换第 145-161 行：

```rust
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ===== 搜索栏（Ctrl+F 激活） =====
            if self.search.visible {
                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // 查找标签
                            ui.label(egui::RichText::new("🔍 查找:").size(13.0));

                            // 查找输入框
                            let search_id = egui::Id::new("search_query_input");
                            let mut search_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.search.query)
                                    .id(search_id)
                                    .desired_width(200.0)
                                    .font(egui::TextStyle::Monospace),
                            );

                            // 焦点请求
                            let ctx_clone = ui.ctx().clone();
                            if self.search.focus_search {
                                ctx_clone.memory_mut(|m| m.request_focus(search_id));
                                self.search.focus_search = false;
                            }

                            // 查询变化时重新搜索
                            if search_resp.changed() {
                                let src = self.state.editor().to_string();
                                self.search.search(&src);
                                // 光标跳到当前匹配
                                if let Some(m) = self.search.current_match_pos() {
                                    let _ = self
                                        .state
                                        .editor_mut()
                                        .set_cursor(Cursor::new(m.line, m.col_start));
                                }
                            }

                            // 匹配计数
                            let count_str = match self.search.current_match {
                                Some(idx) => {
                                    format!("{}/{}", idx + 1, self.search.matches.len())
                                }
                                None => "0/0".to_string(),
                            };
                            ui.label(egui::RichText::new(count_str).size(12.0).weak());

                            ui.separator();

                            // 区分大小写按钮
                            let case_btn = egui::RichText::new("Aa").size(12.0);
                            let case_text = if self.search.case_sensitive {
                                case_btn.clone().strong()
                            } else {
                                case_btn.clone().weak()
                            };
                            if ui
                                .add(egui::Button::new(case_text).min_size(egui::vec2(24.0, 16.0)))
                                .clicked()
                            {
                                self.search.case_sensitive = !self.search.case_sensitive;
                                let src = self.state.editor().to_string();
                                self.search.search(&src);
                            }

                            // 全词匹配按钮
                            let word_btn = egui::RichText::new("ab|").size(12.0);
                            let word_text = if self.search.whole_word {
                                word_btn.clone().strong()
                            } else {
                                word_btn.clone().weak()
                            };
                            if ui
                                .add(egui::Button::new(word_text).min_size(egui::vec2(24.0, 16.0)))
                                .clicked()
                            {
                                self.search.whole_word = !self.search.whole_word;
                                let src = self.state.editor().to_string();
                                self.search.search(&src);
                            }

                            // 上/下一个匹配按钮
                            if ui
                                .add(egui::Button::new("←").min_size(egui::vec2(20.0, 16.0)))
                                .clicked()
                            {
                                if let Some(m) = self.search.prev_match() {
                                    let _ = self.state.editor_mut().set_cursor(Cursor::new(
                                        m.line,
                                        m.col_start,
                                    ));
                                }
                            }
                            if ui
                                .add(egui::Button::new("→").min_size(egui::vec2(20.0, 16.0)))
                                .clicked()
                            {
                                if let Some(m) = self.search.next_match() {
                                    let _ = self.state.editor_mut().set_cursor(Cursor::new(
                                        m.line,
                                        m.col_start,
                                    ));
                                }
                            }

                            // 关闭按钮
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("✕").color(egui::Color32::RED))
                                        .min_size(egui::vec2(20.0, 16.0)),
                                )
                                .clicked()
                            {
                                self.search.close();
                            }
                        });

                        // 替换行
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("🔄 替换:").size(13.0));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search.replace)
                                    .desired_width(200.0)
                                    .font(egui::TextStyle::Monospace),
                            );

                            if ui
                                .add(egui::Button::new("替换").min_size(egui::vec2(48.0, 16.0)))
                                .clicked()
                            {
                                if let Some(m) = self.search.current_match_pos().cloned() {
                                    let range = editor_engine::Selection::new(
                                        Cursor::new(m.line, m.col_start),
                                        Cursor::new(m.line, m.col_end),
                                    );
                                    let replace_text = self.search.replace.clone();
                                    let _ = self.state.editor_mut().apply(
                                        editor_engine::Command::Replace {
                                            range,
                                            text: replace_text,
                                        },
                                    );
                                    let src = self.state.editor().to_string();
                                    self.search.search(&src);
                                    // 跳到下一个匹配
                                    if let Some(next) = self.search.current_match_pos().cloned() {
                                        let _ = self.state.editor_mut().set_cursor(
                                            Cursor::new(next.line, next.col_start),
                                        );
                                    }
                                }
                            }

                            if ui
                                .add(egui::Button::new("全部").min_size(egui::vec2(48.0, 16.0)))
                                .clicked()
                            {
                                let count = self.search.matches.len();
                                // 从后往前替换（避免位置偏移）
                                let mut sorted_matches = self.search.matches.clone();
                                sorted_matches.sort_by(|a, b| {
                                    b.line
                                        .cmp(&a.line)
                                        .then(b.col_start.cmp(&a.col_start))
                                });
                                let replace_text = self.search.replace.clone();
                                for m in &sorted_matches {
                                    let range = editor_engine::Selection::new(
                                        Cursor::new(m.line, m.col_start),
                                        Cursor::new(m.line, m.col_end),
                                    );
                                    let _ = self.state.editor_mut().apply(
                                        editor_engine::Command::Replace {
                                            range,
                                            text: replace_text.clone(),
                                        },
                                    );
                                }
                                self.state.status_message =
                                    format!("已替换 {count} 处");
                                self.search.close();
                            }
                        });
                    });
            }
            // ===== 搜索栏结束 =====

            // 根据视图模式渲染
            match self.view_mode {
                ViewMode::Source => {
                    source_view::show_source_view(
                        ui,
                        &mut self.state,
                        highlighter,
                        &self.search,
                    );
                }
                ViewMode::Preview => {
                    preview_view::show_preview_view(ui, &mut self.state, &mut self.render_cache);
                }
                ViewMode::Hybrid => {
                    hybrid_view::show_hybrid_view(
                        ui,
                        &mut self.state,
                        highlighter,
                        &mut self.render_cache,
                    );
                }
            }
        });
```

在文件顶部添加必要的导入（`use` 区域）：

```rust
use editor_engine::Cursor;
```

（注意：`Cursor` 目前只在 `source_view.rs` 中被导入。现在 `main.rs` 也需要它来移动光标到匹配位置。需要把 `use editor_engine::Cursor;` 添加到 `main.rs` 顶部。）

- [ ] **步骤 2：编译验证**

运行：
```powershell
cargo check -p zdown-app
```
预期：编译错误——`source_view::show_source_view` 签名尚未更新，参数不匹配。先确认搜索栏 UI 部分本身无错误，下一步修改 `source_view.rs` 签名。

- [ ] **步骤 3：Commit**

```bash
git add crates/zdown-app/src/main.rs
git commit -m "feat(search): render search bar UI in CentralPanel

- Two-row layout: find row + replace row
- Buttons: Aa (case), ab| (whole word), arrows (nav), replace, replace all, close
- Match count display with current/total format
- Auto-search on query change, option toggle

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 4：匹配高亮绘制

**文件：**
- 修改：`crates/zdown-app/src/source_view.rs`

- [ ] **步骤 1：更新 `show_source_view` 签名**

将第 17 行的函数签名从：

```rust
pub fn show_source_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
) {
```

改为：

```rust
pub fn show_source_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
) {
```

在文件顶部（第 13 行 `use crate::editor_state::EditorState;` 之后）添加导入：

```rust
use crate::search_state::SearchState;
```

- [ ] **步骤 2：更新 `render_text_with_cursor` 调用**

将第 54 行的调用：

```rust
                render_text_with_cursor(ui, &src, state.editor().cursor, highlighter);
```

改为：

```rust
                render_text_with_cursor(ui, &src, state.editor().cursor, highlighter, search);
```

- [ ] **步骤 3：更新 `render_text_with_cursor` 签名**

将第 61 行函数签名从：

```rust
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
) {
```

改为：

```rust
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
) {
```

- [ ] **步骤 4：实现匹配高亮绘制逻辑**

在 `render_text_with_cursor` 函数内部，在变量 `let row_height = ...`（第 74 行）之后添加辅助函数：

```rust
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

    // 收集当前行的匹配范围（用于高亮绘制）
    fn line_match_ranges(search: &SearchState, line_idx: usize) -> Vec<(usize, usize, bool)> {
        let mut ranges: Vec<(usize, usize, bool)> = Vec::new();
        if !search.visible || search.matches.is_empty() {
            return ranges;
        }
        let current_idx = search.current_match;
        for (i, m) in search.matches.iter().enumerate() {
            if m.line == line_idx {
                let is_current = current_idx == Some(i);
                ranges.push((m.col_start, m.col_end, is_current));
            }
        }
        ranges
    }
```

- [ ] **步骤 5：在 highlighter 分支中叠加匹配高亮**

在 highlighter 分支（第 76-119 行）的每一行渲染前，计算该行的匹配范围。

在第 78 行 `for (line_idx, line) in lines.iter().enumerate()` 循环开始处（第 79 行 `let (rect, _) = ui.allocate_at_least(...)` 之前）添加：

```rust
            for (line_idx, line) in lines.iter().enumerate() {
                let match_ranges = line_match_ranges(search, line_idx);
```

然后在光标绘制之后、高亮文本绘制之前，添加匹配高亮背景的绘制。具体地，在第 101-102 行（光标矩形绘制结束）之后，在第 104 行（`// 绘制高亮文本`）之前插入：

```rust
                // 绘制匹配高亮背景
                for &(col_start, col_end, is_current) in &match_ranges {
                    let m_prefix: String = line
                        .iter()
                        .flat_map(|(_, t)| t.chars())
                        .take(col_start)
                        .collect();
                    let m_text: String = line
                        .iter()
                        .flat_map(|(_, t)| t.chars())
                        .skip(col_start)
                        .take(col_end - col_start)
                        .collect();
                    let m_prefix_galley = ui.ctx().fonts_mut(|f| {
                        f.layout_no_wrap(m_prefix, font_id.clone(), egui::Color32::WHITE)
                    });
                    let m_text_galley = ui.ctx().fonts_mut(|f| {
                        f.layout_no_wrap(m_text, font_id.clone(), egui::Color32::WHITE)
                    });
                    let bg_x = rect.min.x + m_prefix_galley.size().x;
                    let bg_w = m_text_galley.size().x;
                    let bg_color = if is_current {
                        egui::Color32::from_rgb(212, 133, 11) // 橙色
                    } else {
                        egui::Color32::from_rgb(107, 76, 18) // 暗黄
                    };
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(bg_x, rect.min.y),
                            egui::vec2(bg_w, row_height),
                        ),
                        0.0,
                        bg_color,
                    );
                }
```

- [ ] **步骤 6：在 fallback 分支（无 highlighter）中添加匹配高亮**

对 fallback 分支（第 121-145 行）做同样处理。

在第 122 行的 `for (line_idx, line) in src.lines().enumerate()` 循环开始处添加：

```rust
            for (line_idx, line) in src.lines().enumerate() {
                let match_ranges = line_match_ranges(search, line_idx);
```

然后在第 139-143 行的文本绘制之前插入匹配高亮背景（与步骤 5 类似的逻辑，但使用 `line` 字符串而非高亮片段）：

在 `let (rect, _) = ui.allocate_at_least(...)` 后、光标绘制后，文本 galley 绘制前添加：

```rust
                // 绘制匹配高亮背景
                for &(col_start, col_end, is_current) in &match_ranges {
                    let m_prefix: String = line.chars().take(col_start).collect();
                    let m_text: String =
                        line.chars().skip(col_start).take(col_end - col_start).collect();
                    let m_prefix_galley = ui.ctx().fonts_mut(|f| {
                        f.layout_no_wrap(m_prefix, font_id.clone(), egui::Color32::WHITE)
                    });
                    let m_text_galley = ui.ctx().fonts_mut(|f| {
                        f.layout_no_wrap(m_text, font_id.clone(), egui::Color32::WHITE)
                    });
                    let bg_x = rect.min.x + m_prefix_galley.size().x;
                    let bg_w = m_text_galley.size().x;
                    let bg_color = if is_current {
                        egui::Color32::from_rgb(212, 133, 11)
                    } else {
                        egui::Color32::from_rgb(107, 76, 18)
                    };
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(bg_x, rect.min.y),
                            egui::vec2(bg_w, row_height),
                        ),
                        0.0,
                        bg_color,
                    );
                }
```

- [ ] **步骤 7：编译验证**

运行：
```powershell
cargo check -p zdown-app
```
预期：编译成功

- [ ] **步骤 8：Commit**

```bash
git add crates/zdown-app/src/source_view.rs
git commit -m "feat(search): add match highlighting in source view

- line_match_ranges helper extracts match spans for current line
- Current match: orange (#d4850b) background
- Other matches: dark yellow (#6b4c12) background
- Backgrounds drawn before text (paint order: bg → text → cursor)
- Both highlighter and fallback branches supported

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 5：Ctrl+F 快捷键 + Enter/Esc 导航

**文件：**
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：在 main.rs 中添加 Ctrl+F、Esc 和 Enter 快捷键**

在 `ui()` 方法中，第 92 行 `menu::handle_shortcuts(...)` 之后、第 94 行 `// 视图模式快捷键` 之前，添加：

```rust
        // 搜索快捷键（需在 handle_shortcuts 之后，避免 Enter 被编辑器消费）
        if self.search.visible {
            // Esc 关闭搜索栏
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.search.close();
            }
            // Enter 在搜索栏可见时跳到下一个匹配（仅当编辑器没有焦点时）
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some(m) = self.search.next_match() {
                    let _ = self
                        .state
                        .editor_mut()
                        .set_cursor(Cursor::new(m.line, m.col_start));
                }
            }
        }

        // Ctrl+F 切换搜索栏
        if !mods.ctrl {
            // mods 尚未定义，在此获取
        }
        let mods = ctx.input(|i| i.modifiers);
        if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.search.visible = !self.search.visible;
            if self.search.visible {
                self.search.focus_search = true;
                // 如果有选中文本，预填搜索词（egui TextEdit 暂不支持获取 selection，跳过）
                // 立即搜索
                let src = self.state.editor().to_string();
                self.search.search(&src);
            } else {
                self.search.close();
            }
        }
```

注意：`let mods = ctx.input(|i| i.modifiers);` 在第 95 行已有定义。Ctrl+F 快捷键需放在该行**之后**，并移除上面代码中重复的 `let mods = ...`。

最终位置关系（在 `ui()` 方法中）：

```
第 92 行: menu::handle_shortcuts(&ctx, &mut self.state, &mut self.confirm);
第 93 行:
第 94 行: (新增) Esc + Enter 处理（当 search.visible 时）
第 95 行: (新增) 空行
第 96 行: let mods = ctx.input(|i| i.modifiers);
第 97 行: (新增) Ctrl+F 处理
第 98 行: (原有) if mods.ctrl && !mods.shift { ... Ctrl+1/2/3 }
```

- [ ] **步骤 2：编译验证**

运行：
```powershell
cargo check -p zdown-app
```
预期：编译成功

- [ ] **步骤 3：Commit**

```bash
git add crates/zdown-app/src/main.rs
git commit -m "feat(search): add Ctrl+F toggle, Esc close, Enter navigation

- Ctrl+F: toggle search bar, auto-search current editor text
- Esc: close search bar when visible
- Enter: navigate to next match when search bar is visible

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 6：边缘情况处理 + 完整性检查

**文件：**
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：标签页切换时关闭搜索栏**

在 `tab_bar::show_tab_bar` 调用（第 118-120 行）之前添加：

```rust
        // 标签栏（多标签页时显示）
        if self.state.tab_count() > 1 {
            let active_before = self.state.active_tab_index();
            tab_bar::show_tab_bar(ui, &mut self.state, &mut self.confirm);
            // 标签页切换时关闭搜索
            if self.state.active_tab_index() != active_before {
                self.search.close();
            }
        }
```

- [ ] **步骤 2：检查 search 传递给所有视图模式**

确认 `show_source_view` 调用处已传递 `&self.search`（任务 3 步骤 1 已处理）。Preview 和 Hybrid 视图暂不传递搜索状态（后续扩展）。

- [ ] **步骤 3：完整编译 + clippy + 测试**

运行：
```powershell
cargo check -p zdown-app
cargo clippy -p zdown-app -- -D warnings
cargo test -p zdown-app
```
预期：全部通过

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/main.rs
git commit -m "feat(search): close search bar on tab switch

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 7：运行完整验证

- [ ] **步骤 1：运行所有测试**

```powershell
cargo test
```
预期：所有测试通过

- [ ] **步骤 2：运行 clippy**

```powershell
cargo clippy -- -D warnings
```
预期：无警告

- [ ] **步骤 3：运行 fmt 检查**

```powershell
cargo fmt -- --check
```
预期：格式正确

- [ ] **步骤 4：最终提交（如有格式修正）**

如 `cargo fmt` 有改动，运行 `cargo fmt` 后提交：
```bash
git add -u && git commit -m "style: cargo fmt"
```
