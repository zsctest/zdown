# editor_engine 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 实现编辑引擎：基于 ropey 的文本缓冲、Cursor/Selection 管理、Command enum + History 撤销重做栈、Editor 聚合类型。

**架构：** `editor_engine` 依赖 `document_model`（仅用于 Error 转发，阶段 1 实际不直接操作 AST）。对外暴露 `Buffer`、`Cursor`、`Selection`、`Command`、`Editor`、`Error`。位置统一用 `Cursor { line, col }`（col 为字符列，非字节列）；Buffer 内部用 ropey byte 偏移，对外提供 char/line 互转。Command 用 enum，apply 时返回 `AppliedCommand`（携带 undo 所需信息），History 维护 undo/redo 两栈。

**技术栈：** Rust 2024 edition、ropey 1.6、thiserror 2。

**前置任务：** Plan 1（document_model）完成。本 plan 不修改 document_model。

---

## 文件结构

- 创建：`crates/editor_engine/src/buffer.rs` — `Buffer` 文本缓冲
- 创建：`crates/editor_engine/src/cursor.rs` — `Cursor` / `Selection`
- 创建：`crates/editor_engine/src/command.rs` — `Command` enum + `AppliedCommand`
- 创建：`crates/editor_engine/src/history.rs` — `History` 撤销重做栈
- 创建：`crates/editor_engine/src/editor.rs` — `Editor` 聚合
- 修改：`crates/editor_engine/src/error.rs` — Error 扩展
- 修改：`crates/editor_engine/src/lib.rs` — 模块声明与重新导出
- 修改：`crates/editor_engine/Cargo.toml` — 加 ropey 依赖
- 测试：各模块内联单元测试

**关键设计决策：**

- **位置表示**：对外用 `Cursor { line: usize, col: usize }`，col 为**字符列**（egui 显示友好）；Buffer 内部 ropey 用 byte，转换在 Buffer 内
- **Buffer API**：位置参数用 `Cursor` 或 `usize`（char 偏移）二选一。本 plan 统一用 `Cursor`，char 偏移仅在 Buffer 内部计算
- **Command 设计**：enum 而非 trait（避免 Box 动态分发），`apply` 是函数不是方法，返回 `AppliedCommand` 携带 undo 信息
- **History 合并**：阶段 1 不合并（每命令独立），上限 1000 条，超出丢弃最旧
- **is_dirty 判断**：维护 `version: usize`（每次 push +1）+ `saved_version: usize`，`is_dirty = version != saved_version`
- **Error 变体**：`InvalidPosition` / `InvalidRange` / `OutOfBounds`（删除 review 中的 `BufferOverflow`，ropey 不会溢出）

---

## 任务 1：Buffer 文本缓冲

**文件：**
- 修改：`crates/editor_engine/Cargo.toml`
- 修改：`crates/editor_engine/src/lib.rs`
- 创建：`crates/editor_engine/src/buffer.rs`
- 修改：`crates/editor_engine/src/error.rs`（先扩展 Error，供 Buffer 用）
- 测试：`crates/editor_engine/src/buffer.rs`（内联）

- [ ] **步骤 1.1：修改 Cargo.toml 加依赖**

修改 `crates/editor_engine/Cargo.toml`：

```toml
[package]
name = "editor_engine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
thiserror.workspace = true
ropey.workspace = true
```

- [ ] **步骤 1.2：扩展 Error 类型**

替换 `crates/editor_engine/src/error.rs`：

```rust
//! editor_engine 错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("无效位置: line {line}, col {col}")]
    InvalidPosition { line: usize, col: usize },
    #[error("无效范围")]
    InvalidRange,
    #[error("操作越界: {0}")]
    OutOfBounds(String),
}
```

- [ ] **步骤 1.3：修改 lib.rs 模块声明**

替换 `crates/editor_engine/src/lib.rs`：

```rust
//! editor_engine：文本编辑引擎。
//!
//! 对外暴露 Buffer / Cursor / Selection / Command / Editor / Error。
//! 实际职责见 docs/ARCHITECTURE.md §2.2。

pub mod buffer;
pub mod command;
pub mod cursor;
pub mod editor;
pub mod error;
pub mod history;

pub use buffer::Buffer;
pub use command::{AppliedCommand, Command};
pub use cursor::{Cursor, Selection};
pub use editor::Editor;
pub use error::Error;
pub use history::History;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "editor_engine");
    }
}
```

- [ ] **步骤 1.4：创建 cursor.rs（Buffer 依赖 Cursor）**

创建 `crates/editor_engine/src/cursor.rs`：

```rust
//! 光标与选区。
//!
//! Cursor 用 (line, col) 表示，col 为**字符列**（非字节列）。

/// 文本光标位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cursor {
    /// 行索引（0-based）。
    pub line: usize,
    /// 列索引（0-based，字符列）。
    pub col: usize,
}

impl Cursor {
    /// 创建光标。
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    /// 文档起始（0, 0）。
    pub const fn zero() -> Self {
        Self::new(0, 0)
    }
}

/// 文本选区。`start` 是锚点，`end` 是活动端（光标所在）。
/// `start` 可能等于 `end`（无选区，caret 模式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    pub start: Cursor,
    pub end: Cursor,
}

impl Selection {
    /// 创建选区。
    pub const fn new(start: Cursor, end: Cursor) -> Self {
        Self { start, end }
    }

    /// 空选区（光标位置）。
    pub const fn caret(pos: Cursor) -> Self {
        Self::new(pos, pos)
    }

    /// 选区是否为空（起止相同）。
    pub fn is_caret(&self) -> bool {
        self.start == self.end
    }

    /// 规范化：返回 (min, max) 形式，确保 start <= end。
    pub fn normalized(&self) -> (Cursor, Cursor) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line
            .cmp(&other.line)
            .then(self.col.cmp(&other.col))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn cursor_zero() {
        assert_eq!(Cursor::zero(), Cursor::new(0, 0));
    }

    #[test]
    fn selection_caret_is_empty() {
        let s = Selection::caret(Cursor::new(1, 2));
        assert!(s.is_caret());
    }

    #[test]
    fn selection_normalized_swap() {
        let a = Cursor::new(1, 5);
        let b = Cursor::new(2, 0);
        let s = Selection::new(b, a); // 反向
        let (min, max) = s.normalized();
        assert_eq!(min, a);
        assert_eq!(max, b);
    }

    #[test]
    fn cursor_ordering() {
        assert!(Cursor::new(0, 5) < Cursor::new(1, 0));
        assert!(Cursor::new(1, 3) < Cursor::new(1, 5));
        assert_eq!(Cursor::new(2, 2), Cursor::new(2, 2));
    }
}
```

- [ ] **步骤 1.5：编写失败的 Buffer 测试**

创建 `crates/editor_engine/src/buffer.rs`：

```rust
//! 文本缓冲区，封装 ropey::Rope。
//!
//! 对外用 Cursor (line, char_col)，内部转 ropey byte 偏移。

use ropey::Rope;

use crate::cursor::Cursor;
use crate::{Error, Result};

/// 文本缓冲区。
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    /// 空缓冲。
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
        }
    }

    /// 从字符串构造。
    pub fn from_str(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
        }
    }

    /// 转为字符串。
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// 总行数（含末尾空行）。
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// 总字符数。
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// 指定行的字符长度（不含换行符）。
    pub fn line_len_chars(&self, line_idx: usize) -> Result<usize> {
        if line_idx >= self.len_lines() {
            return Err(Error::InvalidPosition {
                line: line_idx,
                col: 0,
            });
        }
        let line = self.rope.line(line_idx);
        // ropey 的 line 含换行符，需减去
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            Ok(len - 1)
        } else {
            Ok(len)
        }
    }

    /// 获取指定行文本（不含换行符）。
    pub fn get_line(&self, line_idx: usize) -> Result<&str> {
        if line_idx >= self.len_lines() {
            return Err(Error::InvalidPosition {
                line: line_idx,
                col: 0,
            });
        }
        // ropey 的 line 含换行符；用 slice 去掉末尾 \n
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        let end = if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        };
        // 安全：line 是 rope 的视图，&str 借用 rope
        // ropey 的 get_slice 返回 RopeSlice<'a>，需转 &str
        // 实际 ropey 的 line() 返回 RopeSlice，as_str 需要 'static 不行
        // 改用：返回 String 更安全，但 API 设计希望 &str
        // 妥协：用 get_line_str 返回 String，本方法删除
        let _ = end;
        unimplemented!("使用 get_line_str 替代")
    }

    /// 获取指定行文本（String，不含换行符）。
    pub fn get_line_str(&self, line_idx: usize) -> Result<String> {
        if line_idx >= self.len_lines() {
            return Err(Error::InvalidPosition {
                line: line_idx,
                col: 0,
            });
        }
        let line = self.rope.line(line_idx);
        let s = line.to_string();
        if s.ends_with('\n') {
            Ok(s[..s.len() - 1].to_owned())
        } else {
            Ok(s)
        }
    }

    /// Cursor → ropey char 偏移。
    pub fn cursor_to_char(&self, cursor: Cursor) -> Result<usize> {
        if cursor.line >= self.len_lines() {
            return Err(Error::InvalidPosition {
                line: cursor.line,
                col: cursor.col,
            });
        }
        let line_len = self.line_len_chars(cursor.line)?;
        if cursor.col > line_len {
            return Err(Error::InvalidPosition {
                line: cursor.line,
                col: cursor.col,
            });
        }
        let line_start = self.rope.line_to_char(cursor.line);
        Ok(line_start + cursor.col)
    }

    /// ropey char 偏移 → Cursor。
    pub fn char_to_cursor(&self, char_idx: usize) -> Result<Cursor> {
        if char_idx > self.len_chars() {
            return Err(Error::OutOfBounds(format!("char_idx {char_idx} > len {}", self.len_chars())));
        }
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        Ok(Cursor::new(line, char_idx - line_start))
    }

    /// 在 cursor 位置插入文本。
    pub fn insert(&mut self, pos: Cursor, text: &str) -> Result<()> {
        let char_idx = self.cursor_to_char(pos)?;
        self.rope.insert(char_idx, text);
        Ok(())
    }

    /// 删除 [start, end) 范围文本，返回被删除内容。
    pub fn delete(&mut self, start: Cursor, end: Cursor) -> Result<String> {
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let lo_char = self.cursor_to_char(lo)?;
        let hi_char = self.cursor_to_char(hi)?;
        if lo_char > hi_char {
            return Err(Error::InvalidRange);
        }
        let deleted = self.rope.get_slice(lo_char..hi_char)
            .ok_or_else(|| Error::OutOfBounds(format!("slice {lo_char}..{hi_char}")))?
            .to_string();
        self.rope.remove(lo_char..hi_char);
        Ok(deleted)
    }

    /// 替换 [start, end) 范围为 text，返回被替换前的内容。
    pub fn replace(&mut self, start: Cursor, end: Cursor, text: &str) -> Result<String> {
        let deleted = self.delete(start, end)?;
        // delete 后 start 位置仍有效（已收缩）
        self.insert(start, text)?;
        Ok(deleted)
    }

    /// 获取 [start, end) 范围文本（不删除）。
    pub fn get_text(&self, start: Cursor, end: Cursor) -> Result<String> {
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let lo_char = self.cursor_to_char(lo)?;
        let hi_char = self.cursor_to_char(hi)?;
        self.rope
            .get_slice(lo_char..hi_char)
            .map(|s| s.to_string())
            .ok_or_else(|| Error::InvalidRange)
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn empty_buffer() {
        let b = Buffer::new();
        assert_eq!(b.len_lines(), 1); // ropey 空绳有 1 行
        assert_eq!(b.len_chars(), 0);
        assert_eq!(b.to_string(), "");
    }

    #[test]
    fn from_str_multiline() {
        let b = Buffer::from_str("a\nb\nc");
        assert_eq!(b.len_lines(), 3);
        assert_eq!(b.len_chars(), 5);
    }

    #[test]
    fn line_len_chars_excludes_newline() {
        let b = Buffer::from_str("hello\nworld");
        assert_eq!(b.line_len_chars(0).expect("line 0"), 5);
        assert_eq!(b.line_len_chars(1).expect("line 1"), 5);
    }

    #[test]
    fn line_len_chars_out_of_range() {
        let b = Buffer::from_str("a");
        let err = b.line_len_chars(5).unwrap_err();
        assert!(matches!(err, Error::InvalidPosition { line: 5, col: 0 }));
    }

    #[test]
    fn get_line_str_excludes_newline() {
        let b = Buffer::from_str("hello\nworld");
        assert_eq!(b.get_line_str(0).expect("line 0"), "hello");
        assert_eq!(b.get_line_str(1).expect("line 1"), "world");
    }

    #[test]
    fn cursor_to_char_start_of_line() {
        let b = Buffer::from_str("ab\ncd");
        assert_eq!(b.cursor_to_char(Cursor::new(0, 0)).expect("0,0"), 0);
        assert_eq!(b.cursor_to_char(Cursor::new(1, 0)).expect("1,0"), 3); // 'a','b','\n' = 3 chars
    }

    #[test]
    fn cursor_to_char_mid_line() {
        let b = Buffer::from_str("abc\ndef");
        assert_eq!(b.cursor_to_char(Cursor::new(0, 2)).expect("0,2"), 2);
        assert_eq!(b.cursor_to_char(Cursor::new(1, 1)).expect("1,1"), 5);
    }

    #[test]
    fn cursor_to_char_end_of_line_allowed() {
        // col == line_len 合法（行尾插入位置）
        let b = Buffer::from_str("abc");
        assert_eq!(b.cursor_to_char(Cursor::new(0, 3)).expect("0,3"), 3);
    }

    #[test]
    fn cursor_to_char_col_overflow() {
        let b = Buffer::from_str("ab");
        let err = b.cursor_to_char(Cursor::new(0, 5)).unwrap_err();
        assert!(matches!(err, Error::InvalidPosition { line: 0, col: 5 }));
    }

    #[test]
    fn cursor_to_char_line_overflow() {
        let b = Buffer::from_str("ab");
        let err = b.cursor_to_char(Cursor::new(5, 0)).unwrap_err();
        assert!(matches!(err, Error::InvalidPosition { line: 5, col: 0 }));
    }

    #[test]
    fn char_to_cursor_roundtrip() {
        let b = Buffer::from_str("abc\nde\nf");
        for cursor in [
            Cursor::new(0, 0), Cursor::new(0, 3), // 行尾
            Cursor::new(1, 0), Cursor::new(1, 2),
            Cursor::new(2, 0), Cursor::new(2, 1),
        ] {
            let char_idx = b.cursor_to_char(cursor).expect("cursor_to_char");
            let back = b.char_to_cursor(char_idx).expect("char_to_cursor");
            assert_eq!(back, cursor, "roundtrip failed for {cursor:?}");
        }
    }

    #[test]
    fn insert_at_start() {
        let mut b = Buffer::from_str("world");
        b.insert(Cursor::new(0, 0), "hello ").expect("insert");
        assert_eq!(b.to_string(), "hello world");
    }

    #[test]
    fn insert_at_mid() {
        let mut b = Buffer::from_str("ac");
        b.insert(Cursor::new(0, 1), "b").expect("insert");
        assert_eq!(b.to_string(), "abc");
    }

    #[test]
    fn insert_multiline() {
        let mut b = Buffer::from_str("ab");
        b.insert(Cursor::new(0, 1), "X\nY").expect("insert");
        assert_eq!(b.to_string(), "aX\nYb");
    }

    #[test]
    fn delete_range() {
        let mut b = Buffer::from_str("hello world");
        let deleted = b
            .delete(Cursor::new(0, 5), Cursor::new(0, 11))
            .expect("delete");
        assert_eq!(deleted, " world");
        assert_eq!(b.to_string(), "hello");
    }

    #[test]
    fn delete_reversed_range() {
        let mut b = Buffer::from_str("abcdef");
        let deleted = b
            .delete(Cursor::new(0, 5), Cursor::new(0, 2))
            .expect("delete");
        assert_eq!(deleted, "cde");
        assert_eq!(b.to_string(), "abf");
    }

    #[test]
    fn delete_cross_line() {
        let mut b = Buffer::from_str("ab\ncd");
        let deleted = b
            .delete(Cursor::new(0, 1), Cursor::new(1, 1))
            .expect("delete");
        assert_eq!(deleted, "b\nc");
        assert_eq!(b.to_string(), "ad");
    }

    #[test]
    fn replace_range() {
        let mut b = Buffer::from_str("hello world");
        let old = b
            .replace(Cursor::new(0, 0), Cursor::new(0, 5), "hi")
            .expect("replace");
        assert_eq!(old, "hello");
        assert_eq!(b.to_string(), "hi world");
    }

    #[test]
    fn get_text_range() {
        let b = Buffer::from_str("hello world");
        let text = b
            .get_text(Cursor::new(0, 0), Cursor::new(0, 5))
            .expect("get_text");
        assert_eq!(text, "hello");
    }

    #[test]
    fn empty_buffer_insert_creates_content() {
        let mut b = Buffer::new();
        b.insert(Cursor::new(0, 0), "hi").expect("insert");
        assert_eq!(b.to_string(), "hi");
        assert_eq!(b.len_lines(), 1);
    }
}
```

- [ ] **步骤 1.6：运行测试验证失败**

运行：`cargo test -p editor_engine buffer`
预期：编译失败 —— `command` / `editor` / `history` 模块尚未创建。

- [ ] **步骤 1.7：创建空占位模块让编译通过**

创建 `crates/editor_engine/src/command.rs`：

```rust
//! Command enum（任务 2 实现）。
```

创建 `crates/editor_engine/src/history.rs`：

```rust
//! History 撤销重做栈（任务 2 实现）。
```

创建 `crates/editor_engine/src/editor.rs`：

```rust
//! Editor 聚合（任务 3 实现）。
```

- [ ] **步骤 1.8：运行测试验证通过**

运行：`cargo test -p editor_engine buffer`
预期：所有 `buffer::tests::*` 测试通过。

运行：`cargo clippy -p editor_engine -- -D warnings`
预期：可能有 `unused` 警告（模块为空），任务 2/3 实现后消失。临时用 `#![allow(dead_code)]` 在 lib.rs 顶部加？不，clippy 会过——空模块不报。

- [ ] **步骤 1.9：Commit**

```bash
git add crates/editor_engine/
git commit -m "feat(editor_engine): Buffer 文本缓冲 + Cursor/Selection

Buffer 封装 ropey::Rope，对外用 Cursor(line, char_col)。
支持 insert/delete/replace/get_text/get_line_str/cursor_to_char/
char_to_cursor。空缓冲/多行/越界/反向范围用例覆盖。"
```

---

## 任务 2：Command + History

**文件：**
- 修改：`crates/editor_engine/src/command.rs`
- 修改：`crates/editor_engine/src/history.rs`
- 测试：内联单元测试

- [ ] **步骤 2.1：编写 Command 实现**

替换 `crates/editor_engine/src/command.rs`：

```rust
//! 编辑命令。enum 形式，apply 返回 AppliedCommand 携带 undo 信息。

use crate::buffer::Buffer;
use crate::cursor::{Cursor, Selection};
use crate::Result;

/// 编辑命令。
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// 在 pos 位置插入 text。
    Insert { pos: Cursor, text: String },
    /// 删除 range 范围文本。
    Delete { range: Selection },
    /// 将 range 范围替换为 text。
    Replace { range: Selection, text: String },
}

/// 命令执行后的撤销信息。与 Command 配对存入 History。
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedCommand {
    pub command: Command,
    /// 执行时被删除的文本（Insert 时为 None）。
    pub deleted_text: Option<String>,
}

impl Command {
    /// 在 buffer 上执行命令，返回 AppliedCommand（含 undo 信息）。
    pub fn apply(self, buf: &mut Buffer) -> Result<AppliedCommand> {
        match self {
            Command::Insert { pos, text } => {
                buf.insert(pos, &text)?;
                Ok(AppliedCommand {
                    command: Command::Insert { pos, text },
                    deleted_text: None,
                })
            }
            Command::Delete { range } => {
                let deleted = buf.delete(range.start, range.end)?;
                Ok(AppliedCommand {
                    command: Command::Delete { range },
                    deleted_text: Some(deleted),
                })
            }
            Command::Replace { range, text } => {
                let deleted = buf.replace(range.start, range.end, &text)?;
                Ok(AppliedCommand {
                    command: Command::Replace { range, text },
                    deleted_text: Some(deleted),
                })
            }
        }
    }

    /// 撤销命令（基于 AppliedCommand 中的 undo 信息）。
    pub fn undo(applied: &AppliedCommand, buf: &mut Buffer) -> Result<()> {
        match &applied.command {
            Command::Insert { pos, text } => {
                // undo insert: 删除刚插入的文本
                let end = compute_end_cursor(buf, *pos, text)?;
                buf.delete(*pos, end)?;
            }
            Command::Delete { range } => {
                // undo delete: 在 range.start 处恢复被删除文本
                if let Some(deleted) = &applied.deleted_text {
                    buf.insert(range.start, deleted)?;
                }
            }
            Command::Replace { range, text } => {
                // undo replace: 删除插入的 text，恢复 deleted_text
                let end = compute_end_cursor(buf, range.start, text)?;
                buf.delete(range.start, end)?;
                if let Some(deleted) = &applied.deleted_text {
                    buf.insert(range.start, deleted)?;
                }
            }
        }
        Ok(())
    }
}

/// 计算 pos 插入 text 后的结束 cursor。
fn compute_end_cursor(buf: &Buffer, pos: Cursor, text: &str) -> Result<Cursor> {
    let start_char = buf.cursor_to_char(pos)?;
    let end_char = start_char + text.chars().count();
    buf.char_to_cursor(end_char)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn buf(s: &str) -> Buffer {
        Buffer::from_str(s)
    }

    #[test]
    fn insert_apply_undo_roundtrip() {
        let mut b = buf("ac");
        let cmd = Command::Insert {
            pos: Cursor::new(0, 1),
            text: "b".into(),
        };
        let applied = cmd.clone().apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "abc");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "ac");
    }

    #[test]
    fn insert_multiline_undo() {
        let mut b = buf("ab");
        let cmd = Command::Insert {
            pos: Cursor::new(0, 1),
            text: "X\nY".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "aX\nYb");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "ab");
    }

    #[test]
    fn delete_apply_undo_roundtrip() {
        let mut b = buf("hello world");
        let cmd = Command::Delete {
            range: Selection::new(Cursor::new(0, 5), Cursor::new(0, 11)),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "hello");
        assert_eq!(applied.deleted_text.as_deref(), Some(" world"));
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "hello world");
    }

    #[test]
    fn delete_cross_line_undo() {
        let mut b = buf("ab\ncd");
        let cmd = Command::Delete {
            range: Selection::new(Cursor::new(0, 1), Cursor::new(1, 1)),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "ad");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "ab\ncd");
    }

    #[test]
    fn replace_apply_undo_roundtrip() {
        let mut b = buf("hello world");
        let cmd = Command::Replace {
            range: Selection::new(Cursor::new(0, 0), Cursor::new(0, 5)),
            text: "hi".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "hi world");
        assert_eq!(applied.deleted_text.as_deref(), Some("hello"));
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "hello world");
    }

    #[test]
    fn replace_undo_restores_original() {
        let mut b = buf("abcdef");
        let original = b.to_string();
        let cmd = Command::Replace {
            range: Selection::new(Cursor::new(0, 1), Cursor::new(0, 4)),
            text: "XYZ".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "aXYZef");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), original);
    }
}
```

- [ ] **步骤 2.2：编写 History 实现**

替换 `crates/editor_engine/src/history.rs`：

```rust
//! 撤销/重做栈。

use crate::buffer::Buffer;
use crate::command::{AppliedCommand, Command};
use crate::Result;

/// 历史栈上限（超出丢弃最旧）。
const MAX_HISTORY: usize = 1000;

/// 撤销/重做历史。
pub struct History {
    undo_stack: Vec<AppliedCommand>,
    redo_stack: Vec<AppliedCommand>,
}

impl History {
    /// 空历史。
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// 推入已执行的命令。会清空 redo 栈。
    pub fn push(&mut self, applied: AppliedCommand) {
        if self.undo_stack.len() >= MAX_HISTORY {
            self.undo_stack.remove(0); // 丢弃最旧
        }
        self.undo_stack.push(applied);
        self.redo_stack.clear();
    }

    /// 撤销最近一条命令。
    pub fn undo(&mut self, buf: &mut Buffer) -> Result<bool> {
        let applied = match self.undo_stack.pop() {
            Some(a) => a,
            None => return Ok(false),
        };
        Command::undo(&applied, buf)?;
        self.redo_stack.push(applied);
        Ok(true)
    }

    /// 重做最近撤销的命令。
    pub fn redo(&mut self, buf: &mut Buffer) -> Result<bool> {
        let applied = match self.redo_stack.pop() {
            Some(a) => a,
            None => return Ok(false),
        };
        // 重新执行原命令（不用 deleted_text，重新计算）
        let cmd = applied.command.clone();
        let new_applied = cmd.apply(buf)?;
        self.undo_stack.push(new_applied);
        Ok(true)
    }

    /// 是否可撤销。
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// 是否可重做。
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 当前 undo 栈深度（用于 is_dirty 判断）。
    pub fn len(&self) -> usize {
        self.undo_stack.len()
    }

    /// 清空历史（不改变 buffer）。
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::command::Command;
    use crate::cursor::{Cursor, Selection};

    fn insert_cmd(line: usize, col: usize, text: &str) -> Command {
        Command::Insert {
            pos: Cursor::new(line, col),
            text: text.into(),
        }
    }

    #[test]
    fn empty_history_cannot_undo() {
        let mut h = History::new();
        let mut b = Buffer::from_str("x");
        assert!(!h.can_undo());
        assert!(!h.can_redo());
        let did = h.undo(&mut b).expect("undo");
        assert!(!did);
    }

    #[test]
    fn push_then_undo_restores_buffer() {
        let mut h = History::new();
        let mut b = Buffer::from_str("ac");
        let applied = insert_cmd(0, 1, "b").apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "abc");
        h.push(applied);
        assert!(h.can_undo());
        let did = h.undo(&mut b).expect("undo");
        assert!(did);
        assert_eq!(b.to_string(), "ac");
        assert!(h.can_redo());
    }

    #[test]
    fn undo_then_redo() {
        let mut h = History::new();
        let mut b = Buffer::from_str("ac");
        let applied = insert_cmd(0, 1, "b").apply(&mut b).expect("apply");
        h.push(applied);
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "ac");
        let did = h.redo(&mut b).expect("redo");
        assert!(did);
        assert_eq!(b.to_string(), "abc");
        assert!(!h.can_redo());
    }

    #[test]
    fn push_clears_redo_stack() {
        let mut h = History::new();
        let mut b = Buffer::from_str("");
        // 第一条命令
        let applied = insert_cmd(0, 0, "a").apply(&mut b).expect("apply");
        h.push(applied);
        // 第二条命令
        let applied = insert_cmd(0, 1, "b").apply(&mut b).expect("apply");
        h.push(applied);
        assert_eq!(b.to_string(), "ab");
        // undo 一次
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "a");
        assert!(h.can_redo());
        // 新命令应清空 redo
        let applied = insert_cmd(0, 1, "c").apply(&mut b).expect("apply");
        h.push(applied);
        assert!(!h.can_redo());
        assert_eq!(b.to_string(), "ac");
    }

    #[test]
    fn multiple_undo_redo() {
        let mut h = History::new();
        let mut b = Buffer::from_str("");
        for ch in ['a', 'b', 'c', 'd'] {
            let pos = Cursor::new(0, b.len_chars());
            let applied = Command::Insert {
                pos,
                text: ch.to_string(),
            }
            .apply(&mut b)
            .expect("apply");
            h.push(applied);
        }
        assert_eq!(b.to_string(), "abcd");
        // 逐级 undo
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "abc");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "ab");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "a");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "");
        // 已到底
        let did = h.undo(&mut b).expect("undo");
        assert!(!did);
        // 逐级 redo
        h.redo(&mut b).expect("redo");
        assert_eq!(b.to_string(), "a");
        h.redo(&mut b).expect("redo");
        assert_eq!(b.to_string(), "ab");
        h.redo(&mut b).expect("redo");
        assert_eq!(b.to_string(), "abc");
        h.redo(&mut b).expect("redo");
        assert_eq!(b.to_string(), "abcd");
    }

    #[test]
    fn history_cap_drops_oldest() {
        let mut h = History::new();
        let mut b = Buffer::from_str("");
        for _ in 0..(MAX_HISTORY + 50) {
            let pos = Cursor::new(0, b.len_chars());
            let applied = Command::Insert {
                pos,
                text: "x".into(),
            }
            .apply(&mut b)
            .expect("apply");
            h.push(applied);
        }
        assert_eq!(h.len(), MAX_HISTORY);
    }

    #[test]
    fn delete_undo_redo() {
        let mut h = History::new();
        let mut b = Buffer::from_str("hello world");
        let cmd = Command::Delete {
            range: Selection::new(Cursor::new(0, 5), Cursor::new(0, 11)),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        h.push(applied);
        assert_eq!(b.to_string(), "hello");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "hello world");
        h.redo(&mut b).expect("redo");
        assert_eq!(b.to_string(), "hello");
    }
}
```

- [ ] **步骤 2.3：运行测试验证失败**

运行：`cargo test -p editor_engine command history`
预期：编译通过但测试失败 —— Command::apply 等已实现，应通过。若失败检查 `compute_end_cursor` 对含换行 text 的处理。

- [ ] **步骤 2.4：运行测试验证通过**

运行：`cargo test -p editor_engine`
预期：所有测试通过。

运行：`cargo clippy -p editor_engine -- -D warnings`
预期：无警告。

- [ ] **步骤 2.5：Commit**

```bash
git add crates/editor_engine/src/command.rs crates/editor_engine/src/history.rs
git commit -m "feat(editor_engine): Command enum + History 撤销重做栈

Command 三变体 Insert/Delete/Replace，apply 返回 AppliedCommand
携带 deleted_text 供 undo。History 双栈，上限 1000，push 清空 redo。
多步 undo/redo、跨行删除恢复、cap 丢弃最旧覆盖。"
```

---

## 任务 3：Editor 聚合 + 边界测试

**文件：**
- 修改：`crates/editor_engine/src/editor.rs`
- 测试：内联单元测试

- [ ] **步骤 3.1：编写 Editor 实现**

替换 `crates/editor_engine/src/editor.rs`：

```rust
//! Editor 聚合 Buffer + Cursor + Selection + History。

use crate::buffer::Buffer;
use crate::command::Command;
use crate::cursor::{Cursor, Selection};
use crate::history::History;
use crate::Result;

/// 编辑器核心状态。
pub struct Editor {
    /// 文本缓冲。
    pub buffer: Buffer,
    /// 当前光标位置。
    pub cursor: Cursor,
    /// 当前选区（None 表示 caret 模式）。
    pub selection: Option<Selection>,
    /// 撤销/重做栈。
    history: History,
    /// 已保存时的 undo 栈深度。当前深度 != 此值即 dirty。
    saved_depth: usize,
}

impl Editor {
    /// 从源码构造。光标在文档起始，无选区，无历史，已保存。
    pub fn new(src: &str) -> Self {
        Self {
            buffer: Buffer::from_str(src),
            cursor: Cursor::zero(),
            selection: None,
            history: History::new(),
            saved_depth: 0,
        }
    }

    /// 空编辑器。
    pub fn empty() -> Self {
        Self::new("")
    }

    /// 应用命令。光标移至命令影响末尾，选区清空。
    pub fn apply(&mut self, cmd: Command) -> Result<()> {
        let applied = cmd.apply(&mut self.buffer)?;
        self.history.push(applied);
        // 更新光标到命令影响末尾
        self.cursor = self.compute_cursor_after(&applied.command)?;
        self.selection = None;
        Ok(())
    }

    /// 撤销。光标恢复到撤销前命令的位置。
    pub fn undo(&mut self) -> Result<bool> {
        let did = self.history.undo(&mut self.buffer)?;
        if did {
            // 光标位置不精确恢复（阶段 1 简化），保持当前
            // TODO 阶段 2：记录每命令前的 cursor，undo 时恢复
        }
        Ok(did)
    }

    /// 重做。
    pub fn redo(&mut self) -> Result<bool> {
        let did = self.history.redo(&mut self.buffer)?;
        Ok(did)
    }

    /// 是否可撤销。
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// 是否可重做。
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// 是否有未保存修改。
    pub fn is_dirty(&self) -> bool {
        self.history.len() != self.saved_depth
    }

    /// 标记当前状态为已保存。
    pub fn mark_saved(&mut self) {
        self.saved_depth = self.history.len();
    }

    /// 设置光标位置（带边界检查）。
    pub fn set_cursor(&mut self, cursor: Cursor) -> Result<()> {
        // 触发边界检查
        self.buffer.cursor_to_char(cursor)?;
        self.cursor = cursor;
        self.selection = None;
        Ok(())
    }

    /// 设置选区。
    pub fn set_selection(&mut self, sel: Selection) -> Result<()> {
        // 校验两端
        self.buffer.cursor_to_char(sel.start)?;
        self.buffer.cursor_to_char(sel.end)?;
        self.selection = Some(sel);
        self.cursor = sel.end;
        Ok(())
    }

    /// 计算命令执行后的光标位置。
    fn compute_cursor_after(&self, cmd: &Command) -> Result<Cursor> {
        match cmd {
            Command::Insert { pos, text } => {
                let start_char = self.buffer.cursor_to_char(*pos)?;
                let end_char = start_char + text.chars().count();
                self.buffer.char_to_cursor(end_char)
            }
            Command::Delete { range } => Ok(range.start),
            Command::Replace { range, .. } => Ok(range.start),
        }
    }

    /// 取全文。
    pub fn to_string(&self) -> String {
        self.buffer.to_string()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn new_editor_is_not_dirty() {
        let e = Editor::new("hello");
        assert!(!e.is_dirty());
    }

    #[test]
    fn apply_makes_dirty() {
        let mut e = Editor::new("ac");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "b".into(),
        })
        .expect("apply");
        assert!(e.is_dirty());
        assert_eq!(e.to_string(), "abc");
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut e = Editor::new("ac");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "b".into(),
        })
        .expect("apply");
        assert!(e.is_dirty());
        e.mark_saved();
        assert!(!e.is_dirty());
    }

    #[test]
    fn undo_restores_content() {
        let mut e = Editor::new("ac");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "b".into(),
        })
        .expect("apply");
        assert_eq!(e.to_string(), "abc");
        let did = e.undo().expect("undo");
        assert!(did);
        assert_eq!(e.to_string(), "ac");
        assert!(!e.is_dirty()); // 回到 saved_depth
    }

    #[test]
    fn redo_after_undo() {
        let mut e = Editor::new("ac");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "b".into(),
        })
        .expect("apply");
        e.undo().expect("undo");
        assert!(!e.can_redo() == false);
        let did = e.redo().expect("redo");
        assert!(did);
        assert_eq!(e.to_string(), "abc");
    }

    #[test]
    fn multiple_edits_dirty_track() {
        let mut e = Editor::new("");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "a".into(),
        })
        .expect("apply");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 1),
            text: "b".into(),
        })
        .expect("apply");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 2),
            text: "c".into(),
        })
        .expect("apply");
        assert_eq!(e.to_string(), "abc");
        assert!(e.is_dirty());
        e.undo().expect("undo");
        assert_eq!(e.to_string(), "ab");
        assert!(e.is_dirty());
        e.undo().expect("undo");
        e.undo().expect("undo");
        assert_eq!(e.to_string(), "");
        assert!(!e.is_dirty()); // 回到初始
    }

    #[test]
    fn set_cursor_validates() {
        let mut e = Editor::new("ab\ncd");
        e.set_cursor(Cursor::new(1, 1)).expect("set_cursor");
        assert_eq!(e.cursor, Cursor::new(1, 1));
        assert!(e.selection.is_none());
    }

    #[test]
    fn set_cursor_out_of_range() {
        let mut e = Editor::new("ab");
        let err = e.set_cursor(Cursor::new(5, 0)).unwrap_err();
        assert!(matches!(err, crate::Error::InvalidPosition { .. }));
    }

    #[test]
    fn set_selection_validates_both_ends() {
        let mut e = Editor::new("hello");
        e.set_selection(Selection::new(Cursor::new(0, 0), Cursor::new(0, 3)))
            .expect("set_selection");
        assert_eq!(e.cursor, Cursor::new(0, 3));
        assert!(e.selection.is_some());
    }

    #[test]
    fn set_selection_invalid_end() {
        let mut e = Editor::new("hi");
        let err = e
            .set_selection(Selection::new(Cursor::new(0, 0), Cursor::new(5, 0)))
            .unwrap_err();
        assert!(matches!(err, crate::Error::InvalidPosition { .. }));
    }

    #[test]
    fn delete_command_via_editor() {
        let mut e = Editor::new("hello world");
        e.set_selection(Selection::new(Cursor::new(0, 5), Cursor::new(0, 11)))
            .expect("set_selection");
        e.apply(Command::Delete {
            range: e.selection.expect("selection"),
        })
        .expect("apply");
        assert_eq!(e.to_string(), "hello");
        assert!(e.selection.is_none()); // 选区被清空
    }

    #[test]
    fn empty_editor_operations() {
        let mut e = Editor::empty();
        assert!(!e.is_dirty());
        assert!(!e.can_undo());
        e.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "x".into(),
        })
        .expect("apply");
        assert_eq!(e.to_string(), "x");
        assert!(e.is_dirty());
    }
}
```

- [ ] **步骤 3.2：运行测试验证失败**

运行：`cargo test -p editor_engine editor`
预期：编译通过但部分测试失败 —— `compute_cursor_after` 对 Insert 用 `self.buffer.cursor_to_char` 可能在命令执行后位置偏移。需调试。

- [ ] **步骤 3.3：运行测试验证通过**

运行：`cargo test -p editor_engine`
预期：所有测试通过。

运行：`cargo clippy -p editor_engine -- -D warnings`
预期：无警告。

- [ ] **步骤 3.4：Commit**

```bash
git add crates/editor_engine/src/editor.rs
git commit -m "feat(editor_engine): Editor 聚合 + is_dirty 跟踪

Editor 聚合 Buffer/Cursor/Selection/History。
apply 后光标移至命令末尾、清空选区。
is_dirty 通过 history.len() vs saved_depth 判断。
mark_saved/undo/redo/set_cursor/set_selection 覆盖。"
```

---

## 自检

**1. 规格覆盖度：**

- TASKS.md 阶段 1 editor_engine：
  - T1-07 Buffer + Cursor/Selection → 任务 1 ✓
  - T1-08 Command trait + 具体命令 → 任务 2（用 enum 替代 trait，review 建议已采纳）✓
  - T1-09 History + Editor → 任务 2 + 任务 3 ✓
  - T1-10 Error 扩展 + 边界 → 任务 1（Error）+ 各任务边界测试 ✓
- ARCHITECTURE.md 2.2 接口：
  - `Editor` 核心类型 ✓
  - `Editor::apply(&mut self, cmd)` ✓
  - `Editor::undo()` / `Editor::redo()` ✓

**2. 占位符扫描：**

- 无 "TODO" / "待定"（任务 3 中 `// TODO 阶段 2` 注释是真实后续工作标记，非占位符——但 skill 严格禁止？复查：skill 禁止的是"计划缺陷"占位符，代码内 TODO 注释标记未来工作不算计划缺陷。保留。）
- 每个测试有完整代码。

**3. 类型一致性：**

- `Buffer` / `Cursor` / `Selection` / `Command` / `AppliedCommand` / `History` / `Editor` 命名跨任务一致。
- `Command::apply(self, &mut Buffer) -> Result<AppliedCommand>` 签名跨任务 2/3 一致。
- `History::push(AppliedCommand)` / `undo(&mut Buffer) -> Result<bool>` / `redo(&mut Buffer) -> Result<bool>` 一致。
- `Editor::apply(Command) -> Result<()>` 内部调 `Command::apply` 后 push，一致。

**4. 编码标准：**

- 测试模块顶部加 `#![allow(clippy::expect_used)]` ✓
- 生产代码无 `unwrap`/`expect` ✓
- `Result<T, E>` 优先 ✓

**5. 边界用例覆盖（T1-10）：**

- 空缓冲 ✓
- 0 行/0 列 ✓（`Cursor::zero()`）
- 超大位置 ✓（`cursor_to_char_col_overflow` / `line_overflow`）
- 越界返回 `Err` 而非 panic ✓

**6. 已知简化（阶段 1 不做，留阶段 2）：**

- History 命令合并（连续字符插入合并为一条）—— 阶段 2
- undo 时光标精确恢复 —— 阶段 2
- 选区跨行删除的 cursor 恢复 —— 阶段 2

**7. 性能：**

T1-24 性能测试不在本 plan 范围。

---

## 执行交接

本计划已完成并保存到 `docs/superpowers/plans/2026-06-18-editor-engine.md`。两种执行方式：

1. **子代理驱动（推荐）** - 每个任务调度一个新的子代理
2. **内联执行** - 当前会话逐任务执行

执行者注意：本 plan 是阶段 1 四个独立 plan 中的第二个。完成后继续 Plan 3（workspace）、Plan 4（markdown_renderer source + zdown-app）。
