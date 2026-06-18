//! Editor 聚合 Buffer + Cursor + Selection + History。

use crate::Result;
use crate::buffer::Buffer;
use crate::command::Command;
use crate::cursor::{Cursor, Selection};
use crate::history::History;

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
        let new_cursor = self.compute_cursor_after(&applied.command)?;
        // 若当前 undo 栈深度 < saved_depth，说明已 undo 到 saved 状态之前，
        // 新命令会清空 redo 栈，此后无法通过 redo 回到 saved_depth。
        // 标记 saved_depth 为 MAX 表示永 dirty（只有 mark_saved 才能恢复）。
        if self.history.len() < self.saved_depth {
            self.saved_depth = usize::MAX;
        }
        self.history.push(applied);
        self.cursor = new_cursor;
        self.selection = None;
        Ok(())
    }

    /// 撤销。
    pub fn undo(&mut self) -> Result<bool> {
        let did = self.history.undo(&mut self.buffer)?;
        Ok(did)
    }

    /// 重做。
    pub fn redo(&mut self) -> Result<bool> {
        let did = self.history.redo(&mut self.buffer)?;
        Ok(did)
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn is_dirty(&self) -> bool {
        self.history.len() != self.saved_depth
    }

    pub fn mark_saved(&mut self) {
        self.saved_depth = self.history.len();
    }

    pub fn set_cursor(&mut self, cursor: Cursor) -> Result<()> {
        self.buffer.cursor_to_char(cursor)?;
        self.cursor = cursor;
        self.selection = None;
        Ok(())
    }

    pub fn set_selection(&mut self, sel: Selection) -> Result<()> {
        self.buffer.cursor_to_char(sel.start)?;
        self.buffer.cursor_to_char(sel.end)?;
        self.selection = Some(sel);
        self.cursor = sel.end;
        Ok(())
    }

    fn compute_cursor_after(&self, cmd: &Command) -> Result<Cursor> {
        match cmd {
            Command::Insert { pos, text } => {
                let start_char = self.buffer.cursor_to_char(*pos)?;
                let end_char = start_char + text.chars().count();
                self.buffer.char_to_cursor(end_char)
            }
            Command::Delete { range } => Ok(range.normalized().0),
            Command::Replace { range, .. } => Ok(range.normalized().0),
        }
    }

    #[allow(clippy::inherent_to_string)]
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
        assert!(!e.is_dirty());
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
        assert!(!e.is_dirty());
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
        let err = e
            .set_cursor(Cursor::new(5, 0))
            .expect_err("set_cursor 应失败");
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
            .expect_err("set_selection 应失败");
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
        assert!(e.selection.is_none());
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

    #[test]
    fn delete_reversed_selection_cursor_valid() {
        let mut e = Editor::new("hello world");
        e.set_selection(Selection::new(Cursor::new(0, 11), Cursor::new(0, 5)))
            .expect("set_selection");
        e.apply(Command::Delete {
            range: e.selection.expect("selection"),
        })
        .expect("apply");
        assert_eq!(e.to_string(), "hello");
        // cursor 应在 (0,5)（normalized lo），不是 (0,11)
        assert_eq!(e.cursor, Cursor::new(0, 5));
    }

    #[test]
    fn is_dirty_after_undo_then_new_edit() {
        let mut e = Editor::new("");
        e.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "a".into(),
        })
        .expect("apply");
        e.mark_saved();
        assert!(!e.is_dirty());
        e.undo().expect("undo");
        assert!(e.is_dirty()); // 偏离 saved "a"（len=0 != saved_depth=1）
        e.apply(Command::Insert {
            pos: Cursor::new(0, 0),
            text: "b".into(),
        })
        .expect("apply");
        assert!(e.is_dirty()); // 新内容 "b" != saved "a"，应 dirty
    }
}
