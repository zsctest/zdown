//! 编辑命令。enum 形式，apply 返回 AppliedCommand 携带 undo 信息。

use crate::Result;
use crate::buffer::Buffer;
use crate::cursor::{Cursor, Selection};

/// 编辑命令。
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// 在 pos 位置插入 text。
    Insert { pos: Cursor, text: String },
    /// 删除 range 范围文本。
    Delete { range: Selection },
    /// 将 range 范围替换为 text。
    Replace { range: Selection, text: String },
    /// 替换整个 buffer 内容为 text。撤销时恢复原全部内容。
    ReplaceAll { text: String },
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
            Command::ReplaceAll { text } => {
                let deleted = buf.to_string();
                if buf.len_chars() > 0 {
                    let end = buf.char_to_cursor(buf.len_chars())?;
                    buf.delete(Cursor::zero(), end)?;
                }
                if !text.is_empty() {
                    buf.insert(Cursor::zero(), &text)?;
                }
                Ok(AppliedCommand {
                    command: Command::ReplaceAll { text },
                    deleted_text: Some(deleted),
                })
            }
        }
    }

    /// 撤销命令（基于 AppliedCommand 中的 undo 信息）。
    pub fn undo(applied: &AppliedCommand, buf: &mut Buffer) -> Result<()> {
        match &applied.command {
            Command::Insert { pos, text } => {
                let end = compute_end_cursor(buf, *pos, text)?;
                buf.delete(*pos, end)?;
            }
            Command::Delete { range } => {
                let (lo, _) = range.normalized();
                if let Some(deleted) = &applied.deleted_text {
                    buf.insert(lo, deleted)?;
                }
            }
            Command::Replace { range, text } => {
                let (lo, _) = range.normalized();
                let end = compute_end_cursor(buf, lo, text)?;
                buf.delete(lo, end)?;
                if let Some(deleted) = &applied.deleted_text {
                    buf.insert(lo, deleted)?;
                }
            }
            Command::ReplaceAll { .. } => {
                if buf.len_chars() > 0 {
                    let end = buf.char_to_cursor(buf.len_chars())?;
                    buf.delete(Cursor::zero(), end)?;
                }
                if let Some(deleted) = &applied.deleted_text {
                    buf.insert(Cursor::zero(), deleted)?;
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

    #[test]
    fn replace_reversed_range_undo() {
        let mut b = buf("abcdef");
        let cmd = Command::Replace {
            range: Selection::new(Cursor::new(0, 5), Cursor::new(0, 2)),
            text: "X".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "abXf");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "abcdef");
    }

    #[test]
    fn replace_all_apply_replaces_content() {
        let mut b = buf("hello\nworld\n");
        let cmd = Command::ReplaceAll {
            text: "new\ncontent".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "new\ncontent");
        assert_eq!(applied.deleted_text.as_deref(), Some("hello\nworld\n"));
    }

    #[test]
    fn replace_all_undo_restores_content() {
        let mut b = buf("original text");
        let cmd = Command::ReplaceAll {
            text: "replaced".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "replaced");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "original text");
    }

    #[test]
    fn replace_all_on_empty_buffer() {
        let mut b = buf("");
        let cmd = Command::ReplaceAll {
            text: "hello".into(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "hello");
        assert_eq!(applied.deleted_text.as_deref(), Some(""));
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "");
    }

    #[test]
    fn replace_all_to_empty() {
        let mut b = buf("some content");
        let cmd = Command::ReplaceAll {
            text: String::new(),
        };
        let applied = cmd.apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "some content");
    }

    #[test]
    fn replace_all_redo_after_undo() {
        let mut b = buf("version 1");
        let cmd = Command::ReplaceAll {
            text: "version 2".into(),
        };
        let applied = cmd.clone().apply(&mut b).expect("apply");
        assert_eq!(b.to_string(), "version 2");
        Command::undo(&applied, &mut b).expect("undo");
        assert_eq!(b.to_string(), "version 1");
        // Re-apply (redo)
        let re_applied = cmd.apply(&mut b).expect("re-apply");
        assert_eq!(b.to_string(), "version 2");
        Command::undo(&re_applied, &mut b).expect("undo redo");
        assert_eq!(b.to_string(), "version 1");
    }
}
