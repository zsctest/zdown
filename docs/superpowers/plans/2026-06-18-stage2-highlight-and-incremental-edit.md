# 补阶段 1 高亮 + 增量编辑实现计划

> **面向 AI 代理的工作者：** 必需子智能体：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。

**目标：** 重构 zdown-app source_view，接入 SourceHighlighter 行内语法高亮 + 完全自绘文本 + 事件监听增量 Command，恢复 undo 历史。

**架构：** source_view 不再用 `TextEdit::multiline`，改为完全自绘：用 `ui.input(|i| i.events.clone())` 监听键盘事件 + `ui.painter` 绘制高亮文本与光标矩形。输入事件转为 `editor_engine::Command`（Insert/Delete），通过 EditorState.apply 推入历史。高亮用 SourceHighlighter 全文高亮，按行绘制。`SourceHighlighter` 实例缓存到 `ZdownApp` 字段避免每帧重建。

**技术栈：** Rust 2024 edition、egui 0.34、editor_engine（path）、markdown_renderer SourceHighlighter（path）。

**前置任务：** Plan 1（markdown_renderer AST 渲染）+ Plan 2（视图模式切换）完成。

---

## 文件结构

- 修改：`crates/zdown-app/src/source_view.rs` — 行内高亮 + 增量编辑（完全自绘）
- 修改：`crates/zdown-app/src/main.rs` — ZdownApp 加 SourceHighlighter 缓存字段

**关键设计决策：**

- **完全自绘**：不用 TextEdit，用 `ui.painter` 绘制文本 + 光标矩形，避免双重输入
- **事件监听**：`ui.input(|i| i.events.clone())` 获取键盘事件，转 Command
- **高亮缓存**：`SourceHighlighter` 实例缓存到 `ZdownApp`，避免每帧 `SourceHighlighter::new()` 重复加载语法集
- **光标渲染**：用 `ui.painter.rect` 绘制闪烁矩形，位置由字符索引 + 字体度量计算
- **选区**：阶段 2 暂不实现选区编辑（仅 caret 模式），阶段 3 加
- **Tab/Shift+方向键**：阶段 2 不响应（Tab 焦点跳转，Shift+方向键选区留阶段 3）

---

## 任务 1：行内高亮 + 完全自绘文本（只读）

**文件：**
- 修改：`crates/zdown-app/src/source_view.rs`
- 修改：`crates/zdown-app/src/main.rs`（加 SourceHighlighter 缓存）

**注意：** 本任务实现的 source_view 只读（不处理输入），下一任务加输入处理。阶段 1 的整体替换编辑逻辑会被覆盖丢失，阶段 1 编辑能力在任务 2 才恢复。若需保留阶段 1 编辑能力到任务 2 完成，可先在本任务保留阶段 1 的 TextEdit 整体替换 + 加高亮，任务 2 再改为完全自绘。**本 plan 选择直接覆盖**（任务 1 只读 + 任务 2 自绘编辑），简化中间态。

- [ ] **步骤 1.1：main.rs 加 SourceHighlighter 缓存字段**

修改 `crates/zdown-app/src/main.rs`，在 `ZdownApp` 加 `highlighter` 字段：

```rust
#[derive(Default)]
struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    last_title: String,
    /// 缓存 SourceHighlighter 避免每帧重建。
    /// Default::default() 会失败（SourceHighlighter 无 Default），
    /// 改用 once_cell 或在 main() 中初始化。这里用 Option + 首次惰性初始化。
    highlighter: Option<markdown_renderer::SourceHighlighter>,
}
```

在 `ZdownApp::ui` 开头惰性初始化：

```rust
if self.highlighter.is_none() {
    self.highlighter = markdown_renderer::SourceHighlighter::new().ok();
}
let highlighter = self.highlighter.as_ref();
```

修改 `source_view::show_source_view` 调用，传 highlighter：

```rust
ViewMode::Source => source_view::show_source_view(ui, &mut self.state, highlighter),
```

- [ ] **步骤 1.2：source_view.rs 实现只读高亮渲染**

替换 `crates/zdown-app/src/source_view.rs`：

```rust
//! 源码编辑视图。
//!
//! 阶段 2：完全自绘 + 行内语法高亮 + 事件监听增量编辑。
//! 本任务：只读高亮渲染（任务 2 加输入处理）。

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;

/// 渲染源码编辑视图。
pub fn show_source_view(ui: &mut egui::Ui, state: &mut EditorState, highlighter: Option<&SourceHighlighter>) {
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

            // 高亮文本列
            ui.vertical(|ui| {
                highlight_source(ui, &src, highlighter);
            });
        });
    });
}

/// 用 SourceHighlighter 高亮源码并逐行绘制。
fn highlight_source(ui: &mut egui::Ui, src: &str, highlighter: Option<&SourceHighlighter>) {
    if let Some(h) = highlighter {
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

- [ ] **步骤 1.3：编译验证 + smoke**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：通过。

- [ ] **步骤 1.4：Commit**

```bash
git add crates/zdown-app/src/source_view.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): 源码视图行内语法高亮（只读）

完全自绘（不用 TextEdit），用 SourceHighlighter 全文高亮逐行绘制。
SourceHighlighter 实例缓存到 ZdownApp 字段，避免每帧重建。
本任务只读，任务 2 加输入处理。"
```

---

## 任务 2：事件监听 + 增量编辑命令 + 光标渲染

**文件：**
- 修改：`crates/zdown-app/src/source_view.rs`

- [ ] **步骤 2.1：加输入处理 + 光标渲染**

替换 `crates/zdown-app/src/source_view.rs`，在任务 1 基础上加：

```rust
//! 源码编辑视图。
//!
//! 阶段 2：完全自绘 + 行内语法高亮 + 事件监听增量编辑。
//!
//! 实现：
//! - ui.input(|i| i.events.clone()) 监听键盘事件
//! - 事件转 editor_engine::Command（Insert/Delete）推入历史
//! - ui.painter 绘制光标矩形（精确像素定位）

use eframe::egui;
use markdown_renderer::SourceHighlighter;

use crate::editor_state::EditorState;
use editor_engine::{Command, Cursor};

/// 渲染源码编辑视图。
pub fn show_source_view(ui: &mut egui::Ui, state: &mut EditorState, highlighter: Option<&SourceHighlighter>) {
    let src = state.editor.to_string();

    // 先处理输入事件（更新 editor），再渲染（避免一帧延迟）
    // 注意：提前 clone ctx 避免 ui 借用冲突
    let ctx = ui.ctx().clone();
    let input_response = ui.interact(
        ui.max_rect(),
        egui::Id::new("source_view_input"),
        egui::Sense::click_and_drag(),
    );
    if input_response.has_focus() {
        handle_input(&ctx, state);
    }
    // 点击获取焦点（egui 0.34 的 request_focus API 可能改名，按实际调整）
    if input_response.clicked() {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("source_view_input")));
    }

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

            // 高亮文本 + 光标
            ui.vertical(|ui| {
                render_text_with_cursor(ui, &src, state.editor.cursor, highlighter);
            });
        });
    });
}

/// 处理输入事件，转为 editor_engine::Command。
fn handle_input(ctx: &egui::Context, state: &mut EditorState) {
    let events = ctx.input(|i| i.events.clone());
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
                if let Some(prev) = prev_cursor(&state.editor.buffer, cursor) {
                    let _ = state.apply(Command::Delete {
                        range: editor_engine::Selection::new(prev, cursor),
                    });
                    let _ = state.editor.set_cursor(prev);
                }
            }
            egui::Event::Key {
                key: egui::Key::Delete,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                if let Some(next) = next_cursor(&state.editor.buffer, cursor) {
                    let _ = state.apply(Command::Delete {
                        range: editor_engine::Selection::new(cursor, next),
                    });
                    let _ = state.editor.set_cursor(next);
                }
            }
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                ..
            } => {
                let cursor = state.editor.cursor;
                // 先 apply 插入换行，成功后才 set_cursor
                if state.apply(Command::Insert { pos: cursor, text: "\n".into() }).is_ok() {
                    // 插入成功后 buffer 已更新，cursor.line + 1 有效
                    let _ = state.editor.set_cursor(Cursor::new(cursor.line + 1, 0));
                }
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
                    // clamp col 到目标行长度
                    let target_line = cursor.line - 1;
                    let max_col = state.editor.buffer.line_len_chars(target_line).unwrap_or(0);
                    let new_col = cursor.col.min(max_col);
                    let _ = state.editor.set_cursor(Cursor::new(target_line, new_col));
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
                    let target_line = cursor.line + 1;
                    let max_col = state.editor.buffer.line_len_chars(target_line).unwrap_or(0);
                    let new_col = cursor.col.min(max_col);
                    let _ = state.editor.set_cursor(Cursor::new(target_line, new_col));
                }
            }
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed: true,
                ..
            } => {
                // 阶段 2：拦截 Tab 不处理（避免焦点跳转），阶段 3 实现 Tab 缩进
            }
            _ => {}
        }
    }
}

/// 渲染高亮文本 + 光标矩形。
///
/// 注意：egui 0.34 的 `painter.galley` / `fonts(|f| f.layout_no_wrap(...))` API
/// 可能略有不同。若编译失败，按错误调整（如 `f.layout(...)` 或 `Shape::galley`）。
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
) {
    // 从 egui style 获取等宽字体字号，避免硬编码
    let font_id = ui.style().text_styles
        .get(&egui::TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| egui::FontId::monospace(14.0));
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

    if let Some(h) = highlighter {
        let lines = h.highlight(src, None);
        for (line_idx, line) in lines.iter().enumerate() {
            let (rect, _) = ui.allocate_at_least(egui::vec2(ui.available_width(), row_height), egui::Sense::hover());

            // 绘制光标矩形（在光标所在行）
            if line_idx == cursor.line {
                // 计算光标 x 位置：光标前所有字符的宽度之和
                let prefix: String = line.iter()
                    .flat_map(|(_, t)| t.chars())
                    .take(cursor.col)
                    .collect();
                let prefix_galley = ui.fonts(|f| f.layout_no_wrap(prefix.clone(), font_id.clone(), egui::Color32::WHITE));
                let cursor_x = rect.min.x + prefix_galley.size().x;
                let cursor_rect = egui::Rect::from_min_size(
                    egui::pos2(cursor_x, rect.min.y),
                    egui::vec2(2.0, row_height),
                );
                ui.painter().rect_filled(cursor_rect, 0.0, egui::Color32::from_rgb(200, 200, 200));
            }

            // 绘制高亮文本
            let mut x = rect.min.x;
            for (style, text) in line {
                let color = egui::Color32::from_rgb(
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                );
                let galley = ui.fonts(|f| f.layout_no_wrap((*text).to_string(), font_id.clone(), color));
                ui.painter().galley(egui::pos2(x, rect.min.y), galley.clone(), color);
                x += galley.size().x;
            }
        }
    } else {
        // fallback：不高亮
        for (line_idx, line) in src.lines().enumerate() {
            let (rect, _) = ui.allocate_at_least(egui::vec2(ui.available_width(), row_height), egui::Sense::hover());
            if line_idx == cursor.line {
                let prefix: String = line.chars().take(cursor.col).collect();
                let prefix_galley = ui.fonts(|f| f.layout_no_wrap(prefix, font_id.clone(), egui::Color32::WHITE));
                let cursor_x = rect.min.x + prefix_galley.size().x;
                let cursor_rect = egui::Rect::from_min_size(
                    egui::pos2(cursor_x, rect.min.y),
                    egui::vec2(2.0, row_height),
                );
                ui.painter().rect_filled(cursor_rect, 0.0, egui::Color32::from_rgb(200, 200, 200));
            }
            let galley = ui.fonts(|f| f.layout_no_wrap(line.to_string(), font_id.clone(), egui::Color32::WHITE));
            ui.painter().galley(rect.min, galley, egui::Color32::WHITE);
        }
    }
}

/// 计算光标前一个位置。
/// 注意：Plan 4 任务 2.2 会改为 `pub(crate)` 供 hybrid_view 复用。
fn prev_cursor(buffer: &editor_engine::Buffer, cursor: Cursor) -> Option<Cursor> {
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
/// 注意：Plan 4 任务 2.2 会改为 `pub(crate)` 供 hybrid_view 复用。
fn next_cursor(buffer: &editor_engine::Buffer, cursor: Cursor) -> Option<Cursor> {
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
```

> **注意：** 任务 1 的 `highlight_source` 函数在本任务被 `render_text_with_cursor` 替代，执行者应删除 `highlight_source`（避免死代码）。

- [ ] **步骤 2.2：编译验证 + smoke + clippy**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app && cargo clippy -p zdown-app --all-targets -- -D warnings`
预期：通过。

> **执行者注意：** egui 0.34 的 `painter.galley` 签名可能是 `painter.add(Shape::galley(...))` 或 `painter.galley(pos, galley, color)`。若编译失败，按错误调整。`fonts(|f| f.layout_no_wrap(...))` 也可能 API 略不同（如 `f.layout(...)`），按实际调整。

- [ ] **步骤 2.3：Commit**

```bash
git add crates/zdown-app/src/source_view.rs
git commit -m "feat(zdown-app): 增量编辑命令 + 光标渲染（完全自绘）

完全不用 TextEdit，用 ui.input 事件监听 + ui.painter 自绘：
- Text → Insert Command
- Backspace/Delete → Delete Command
- Enter → Insert('\n')
- 方向键 → set_cursor（clamp col 到目标行长度）
- 光标矩形用 painter.rect_filled 绘制（精确像素定位）
undo/redo 通过 EditorState 恢复。

已知简化（阶段 3）：
- 选区编辑（Shift+方向键）
- Tab 缩进
- 点击定位光标（鼠标点击）"
```

---

## 任务 3：全量验证

- [ ] **步骤 3.1：fmt + clippy + test + build + smoke**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
ZDOWN_SMOKE=1 cargo run -p zdown-app
```

- [ ] **步骤 3.2：本地手动验证**

- 源码模式显示语法高亮
- 输入字符，高亮实时更新
- 光标矩形显示在正确位置
- Backspace/Delete 工作
- Ctrl+Z 撤销，Ctrl+Y 重做（undo 历史恢复）
- 方向键移动光标（含跨行 clamp）
- Enter 换行

- [ ] **步骤 3.3：Commit（如有修复）**

```bash
git add -A
git commit -m "chore: Plan 3 验证通过"
```

---

## 自检

**1. 规格覆盖度：**

- ROADMAP 阶段 2 "补阶段 1 高亮降级"：源码模式行内语法高亮 → 任务 1 ✓
- ROADMAP 阶段 2 "补阶段 1 增量编辑"：基于光标事件的增量 Command → 任务 2 ✓
- 恢复 undo 历史 → 任务 2 ✓

**2. 占位符扫描：**

- 每个步骤含完整代码
- 无"TODO"/"待定"

**3. 类型一致性：**

- `Command::Insert { pos, text }` / `Command::Delete { range }` 与 editor_engine 一致
- `Cursor::new(line, col)` 与 editor_engine 一致
- `SourceHighlighter::highlight(&str, Option<&str>)` 与阶段 1 一致
- `show_source_view(ui, state, highlighter: Option<&SourceHighlighter>)` 签名跨任务一致

**4. 已知简化（阶段 3）：**

- 选区编辑（Shift+方向键）
- Tab 缩进
- 点击定位光标（鼠标点击坐标 → cursor 转换）
- 完全自绘的滚动同步（行号与文本滚动一致）

---

## 执行交接

本计划已保存到 `docs/superpowers/plans/2026-06-18-stage2-highlight-and-incremental-edit.md`。

执行者注意：完成后继续 Plan 4（hybrid + 渲染缓存）。
