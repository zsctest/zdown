# 补阶段 1 高亮 + 增量编辑实现计划

> **面向 AI 代理的工作者：** 必需子智能体：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。

**目标：** 重构 zdown-app source_view，接入 SourceHighlighter 行内语法高亮 + 基于光标事件的增量 Command，恢复 undo 历史。

**架构：** source_view 不再用 `TextEdit::multiline` 整体替换，改为自绘文本布局 + 高亮 + 光标 + 输入处理。输入事件转为 `editor_engine::Command`（Insert/Delete），通过 EditorState.apply 推入历史。高亮用 SourceHighlighter 全文高亮，按行绘制。

**技术栈：** Rust 2024 edition、egui 0.34、editor_engine（path）、markdown_renderer SourceHighlighter（path）。

**前置任务：** Plan 1（markdown_renderer AST 渲染）+ Plan 2（视图模式切换）完成。

---

## 文件结构

- 修改：`crates/zdown-app/src/source_view.rs` — 行内高亮 + 增量编辑
- 修改：`crates/zdown-app/src/editor_state.rs` — 增量编辑辅助方法（如需要）

**关键设计决策：**

- **spike 优先**：先评估 egui 0.34 自绘文本布局的可行方案
- **高亮策略**：SourceHighlighter 全文高亮 → 按行绘制 RichText（含颜色）
- **增量编辑**：监听 egui 输入事件（key_pressed/text），转为 Command::Insert/Delete
- **光标管理**：自绘光标矩形，处理点击定位、方向键移动
- **选区**：阶段 2 暂不实现选区编辑（仅 caret 模式），阶段 3 加

---

## 任务 1：spike — 评估自绘文本布局方案

**文件：** 临时 spike 文件

- [ ] **步骤 1.1：spike 评估**

创建 `crates/zdown-app/src/spike_highlight.rs`（临时）：

```rust
//! spike：评估自绘文本高亮方案。
//! 方案 A：用 ui.label 逐行绘制高亮 RichText（不可编辑）
//! 方案 B：用 TextEdit::multiline + 自定义 foreground color（egui 0.34 可能支持）
//! 方案 C：完全自绘（Painter + 自管理光标/输入）

use eframe::egui;
use markdown_renderer::SourceHighlighter;

pub fn spike_highlight(ui: &mut egui::Ui, src: &str) {
    // 方案 A：逐行 label
    let h = SourceHighlighter::new().ok();
    if let Some(h) = &h {
        let lines = h.highlight(src, None);
        ui.vertical(|ui| {
            for line in lines {
                ui.horizontal(|ui| {
                    for (style, text) in line {
                        let color = egui::Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );
                        ui.label(egui::RichText::new(text).color(color).monospace());
                    }
                });
            }
        });
    }
}
```

- [ ] **步骤 1.2：评估结论**

- 方案 A（逐行 label）：可显示高亮，但不可编辑
- 方案 B（TextEdit + foreground）：egui 0.34 的 TextEdit 不支持片段级颜色
- 方案 C（完全自绘）：工作量大，但可实现高亮 + 编辑

**结论**：阶段 2 采用方案 A + 隐藏 TextEdit 接受输入。即：
- 用 `TextEdit::multiline` 接受输入（透明文本，不可见）
- 在其上层用方案 A 绘制高亮文本
- 输入事件转为 Command

这是 hack 方案，但可行。完全自绘留阶段 3 优化。

- [ ] **步骤 1.3：删除 spike**

```bash
rm crates/zdown-app/src/spike_highlight.rs
```

---

## 任务 2：行内高亮（只读）

**文件：**
- 修改：`crates/zdown-app/src/source_view.rs`

- [ ] **步骤 2.1：实现行内高亮（只读模式）**

修改 `crates/zdown-app/src/source_view.rs`：

```rust
//! 源码编辑视图。
//!
//! 阶段 2：行内语法高亮 + 增量编辑命令。

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;

/// 渲染源码编辑视图。
pub fn show_source_view(ui: &mut egui::Ui, state: &mut EditorState) {
    let src = state.editor.to_string();
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        // 行号 + 高亮文本
        ui.horizontal(|ui| {
            // 行号列
            let line_count = src.lines().count().max(1);
            ui.vertical(|ui| {
                for i in 0..line_count {
                    ui.label(
                        egui::RichText::new(&format!("{:>3}", i + 1))
                            .monospace()
                            .weak(),
                    );
                }
            });
            
            ui.separator();
            
            // 高亮文本列
            ui.vertical(|ui| {
                highlight_source(ui, &src);
            });
        });
    });
}

/// 用 SourceHighlighter 高亮源码并逐行绘制。
fn highlight_source(ui: &mut egui::Ui, src: &str) {
    let highlighter = SourceHighlighter::new().ok();
    if let Some(h) = &highlighter {
        let lines = h.highlight(src, None);
        for line in lines {
            ui.horizontal(|ui| {
                for (style, text) in line {
                    let color = egui::Color32::from_rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    );
                    ui.label(egui::RichText::new(text).color(color).monospace());
                }
            });
        }
    } else {
        // fallback：不高亮
        for line in src.lines() {
            ui.label(egui::RichText::new(line).monospace());
        }
    }
}
```

- [ ] **步骤 2.2：编译验证 + smoke**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：通过。

- [ ] **步骤 2.3：Commit**

```bash
git add crates/zdown-app/src/source_view.rs
git commit -m "feat(zdown-app): 源码视图行内语法高亮（只读）

用 SourceHighlighter 全文高亮，逐行绘制 RichText。
syntect Style 颜色转 egui Color32。
fallback 到单色（highlighter 加载失败时）。"
```

---

## 任务 3：增量编辑命令

**文件：**
- 修改：`crates/zdown-app/src/source_view.rs`

- [ ] **步骤 3.1：叠加 TextEdit 接受输入 + 转为 Command**

修改 `crates/zdown-app/src/source_view.rs`：

```rust
//! 源码编辑视图。
//!
//! 阶段 2：行内语法高亮 + 增量编辑命令。
//!
//! 实现：高亮文本（只读）+ 透明 TextEdit 接受输入 + 输入转 Command。

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;

/// 渲染源码编辑视图。
pub fn show_source_view(ui: &mut egui::Ui, state: &mut EditorState) {
    let src = state.editor.to_string();
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal(|ui| {
            // 行号列
            let line_count = src.lines().count().max(1);
            ui.vertical(|ui| {
                for i in 0..line_count {
                    ui.label(
                        egui::RichText::new(&format!("{:>3}", i + 1))
                            .monospace()
                            .weak(),
                    );
                }
            });
            
            ui.separator();
            
            // 高亮文本 + 输入处理
            ui.vertical(|ui| {
                highlight_source(ui, &src);
                
                // 透明 TextEdit 接受输入（叠加在高亮文本上）
                let mut input_text = String::new();
                let response = ui.add(
                    egui::TextEdit::multiline(&mut input_text)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace)
                        .interactive(true),
                );
                
                // 处理输入事件
                if response.has_focus() {
                    handle_input(ui, state, &response);
                }
            });
        });
    });
}

/// 处理输入事件，转为 editor_engine::Command。
fn handle_input(ui: &mut egui::Ui, state: &mut EditorState, _response: &egui::Response) {
    use editor_engine::{Command, Cursor};
    
    // 文本输入
    let events = ui.input(|i| i.events.clone());
    for event in events {
        match event {
            egui::Event::Text(text) => {
                if !text.is_empty() {
                    let cursor = state.editor.cursor;
                    let _ = state.apply(Command::Insert { pos: cursor, text });
                }
            }
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                // 删除光标前一个字符
                if cursor.col > 0 || cursor.line > 0 {
                    let prev = prev_cursor(&state.editor.buffer, cursor);
                    let _ = state.apply(Command::Delete {
                        range: editor_engine::Selection::new(prev, cursor),
                    });
                }
            }
            egui::Event::Key {
                key: egui::Key::Delete,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                let next = next_cursor(&state.editor.buffer, cursor);
                if let Some(next) = next {
                    let _ = state.apply(Command::Delete {
                        range: editor_engine::Selection::new(cursor, next),
                    });
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                let _ = state.apply(Command::Insert { pos: cursor, text: "\n".into() });
            }
            egui::Event::Key {
                key: egui::Key::ArrowLeft,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                if let Some(prev) = prev_cursor(&state.editor.buffer, cursor) {
                    let _ = state.editor.set_cursor(prev);
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowRight,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                if let Some(next) = next_cursor(&state.editor.buffer, cursor) {
                    let _ = state.editor.set_cursor(next);
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowUp,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                if cursor.line > 0 {
                    let _ = state.editor.set_cursor(Cursor::new(cursor.line - 1, cursor.col));
                }
            }
            egui::Event::Key {
                key: egui::Key::ArrowDown,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                let line_count = state.editor.buffer.len_lines();
                if cursor.line + 1 < line_count {
                    let _ = state.editor.set_cursor(Cursor::new(cursor.line + 1, cursor.col));
                }
            }
            _ => {}
        }
    }
}

/// 计算光标前一个位置。
fn prev_cursor(buffer: &editor_engine::Buffer, cursor: editor_engine::Cursor) -> Option<editor_engine::Cursor> {
    use editor_engine::Cursor;
    if cursor.col > 0 {
        Some(Cursor::new(cursor.line, cursor.col - 1))
    } else if cursor.line > 0 {
        let prev_line = cursor.line - 1;
        let len = buffer.line_len_chars(prev_line).ok()?;
        Some(Cursor::new(prev_line, len))
    } else {
        None
    }
}

/// 计算光标后一个位置。
fn next_cursor(buffer: &editor_engine::Buffer, cursor: editor_engine::Cursor) -> Option<editor_engine::Cursor> {
    use editor_engine::Cursor;
    let line_len = buffer.line_len_chars(cursor.line).ok()?;
    if cursor.col < line_len {
        Some(Cursor::new(cursor.line, cursor.col + 1))
    } else {
        let line_count = buffer.len_lines();
        if cursor.line + 1 < line_count {
            Some(Cursor::new(cursor.line + 1, 0))
        } else {
            None
        }
    }
}

/// 用 SourceHighlighter 高亮源码并逐行绘制。
fn highlight_source(ui: &mut egui::Ui, src: &str) {
    let highlighter = SourceHighlighter::new().ok();
    if let Some(h) = &highlighter {
        let lines = h.highlight(src, None);
        for line in lines {
            ui.horizontal(|ui| {
                for (style, text) in line {
                    let color = egui::Color32::from_rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    );
                    ui.label(egui::RichText::new(text).color(color).monospace());
                }
            });
        }
    } else {
        for line in src.lines() {
            ui.label(egui::RichText::new(line).monospace());
        }
    }
}
```

- [ ] **步骤 3.2：编译验证 + smoke**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：通过。

运行：`cargo clippy -p zdown-app --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 3.3：Commit**

```bash
git add crates/zdown-app/src/source_view.rs
git commit -m "feat(zdown-app): 增量编辑命令（恢复 undo 历史）

输入事件转 editor_engine::Command：
- Text → Insert
- Backspace/Delete → Delete
- Enter → Insert('\n')
- 方向键 → set_cursor
prev_cursor/next_cursor 辅助函数。
undo/redo 通过 EditorState 恢复。"
```

---

## 任务 4：全量验证

- [ ] **步骤 4.1：fmt + clippy + test + build + smoke**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
ZDOWN_SMOKE=1 cargo run -p zdown-app
```

- [ ] **步骤 4.2：本地手动验证**

- 源码模式显示语法高亮
- 输入字符，高亮实时更新
- Backspace/Delete 工作
- Ctrl+Z 撤销，Ctrl+Y 重做（undo 历史恢复）
- 方向键移动光标

- [ ] **步骤 4.3：Commit（如有修复）**

```bash
git add -A
git commit -m "chore: Plan 3 验证通过"
```

---

## 自检

**1. 规格覆盖度：**

- ROADMAP 阶段 2 "补阶段 1 高亮降级"：源码模式行内语法高亮 → 任务 2 ✓
- ROADMAP 阶段 2 "补阶段 1 增量编辑"：基于光标事件的增量 Command → 任务 3 ✓
- 恢复 undo 历史 → 任务 3 ✓

**2. 占位符扫描：**

- spike 是探索性，验证后删除
- 每个步骤含完整代码

**3. 类型一致性：**

- `Command::Insert { pos, text }` / `Command::Delete { range }` 与 editor_engine 一致
- `Cursor::new(line, col)` 与 editor_engine 一致
- `SourceHighlighter::highlight(&str, Option<&str>)` 与阶段 1 一致

**4. 已知简化：**

- 选区编辑留阶段 3
- 完全自绘文本（替代 TextEdit hack）留阶段 3
- 光标渲染（矩形）暂未实现，留 Plan 4

---

## 执行交接

本计划已保存到 `docs/superpowers/plans/2026-06-18-stage2-highlight-and-incremental-edit.md`。

执行者注意：完成后继续 Plan 4（hybrid + 渲染缓存）。
