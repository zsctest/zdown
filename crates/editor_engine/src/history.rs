//! 撤销/重做栈。

use crate::Result;
use crate::buffer::Buffer;
use crate::command::{AppliedCommand, Command};

/// 历史栈上限（超出丢弃最旧）。
const MAX_HISTORY: usize = 1000;

/// 撤销/重做历史。
pub struct History {
    undo_stack: Vec<AppliedCommand>,
    redo_stack: Vec<AppliedCommand>,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// 推入已执行的命令。会清空 redo 栈。
    pub fn push(&mut self, applied: AppliedCommand) {
        if self.undo_stack.len() >= MAX_HISTORY {
            self.undo_stack.remove(0);
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
        let cmd = applied.command.clone();
        let new_applied = cmd.apply(buf)?;
        self.undo_stack.push(new_applied);
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty()
    }

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
        let applied = insert_cmd(0, 0, "a").apply(&mut b).expect("apply");
        h.push(applied);
        let applied = insert_cmd(0, 1, "b").apply(&mut b).expect("apply");
        h.push(applied);
        assert_eq!(b.to_string(), "ab");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "a");
        assert!(h.can_redo());
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
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "abc");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "ab");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "a");
        h.undo(&mut b).expect("undo");
        assert_eq!(b.to_string(), "");
        let did = h.undo(&mut b).expect("undo");
        assert!(!did);
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
