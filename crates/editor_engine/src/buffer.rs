//! 文本缓冲区，封装 ropey::Rope。
//!
//! 对外用 Cursor (line, char_col)，内部转 ropey byte 偏移。

use ropey::Rope;

use crate::cursor::Cursor;
use crate::{Error, Result};

pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
        }
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line_len_chars(&self, line_idx: usize) -> Result<usize> {
        if line_idx >= self.len_lines() {
            return Err(Error::InvalidPosition {
                line: line_idx,
                col: 0,
            });
        }
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            Ok(len - 1)
        } else {
            Ok(len)
        }
    }

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

    pub fn char_to_cursor(&self, char_idx: usize) -> Result<Cursor> {
        if char_idx > self.len_chars() {
            return Err(Error::OutOfBounds(format!(
                "char_idx {char_idx} > len {}",
                self.len_chars()
            )));
        }
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        Ok(Cursor::new(line, char_idx - line_start))
    }

    pub fn insert(&mut self, pos: Cursor, text: &str) -> Result<()> {
        let char_idx = self.cursor_to_char(pos)?;
        self.rope.insert(char_idx, text);
        Ok(())
    }

    pub fn delete(&mut self, start: Cursor, end: Cursor) -> Result<String> {
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let lo_char = self.cursor_to_char(lo)?;
        let hi_char = self.cursor_to_char(hi)?;
        debug_assert!(lo_char <= hi_char);
        let deleted = self
            .rope
            .get_slice(lo_char..hi_char)
            .ok_or_else(|| Error::OutOfBounds(format!("slice {lo_char}..{hi_char}")))?
            .to_string();
        self.rope.remove(lo_char..hi_char);
        Ok(deleted)
    }

    pub fn replace(&mut self, start: Cursor, end: Cursor, text: &str) -> Result<String> {
        let (lo, _) = crate::cursor::Selection::new(start, end).normalized();
        let deleted = self.delete(start, end)?;
        self.insert(lo, text)?;
        Ok(deleted)
    }

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
            .ok_or(Error::InvalidRange)
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
        assert_eq!(b.len_lines(), 1);
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
        let err = b.line_len_chars(5).expect_err("line_len_chars 应失败");
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
        assert_eq!(b.cursor_to_char(Cursor::new(1, 0)).expect("1,0"), 3);
    }

    #[test]
    fn cursor_to_char_mid_line() {
        let b = Buffer::from_str("abc\ndef");
        assert_eq!(b.cursor_to_char(Cursor::new(0, 2)).expect("0,2"), 2);
        assert_eq!(b.cursor_to_char(Cursor::new(1, 1)).expect("1,1"), 5);
    }

    #[test]
    fn cursor_to_char_end_of_line_allowed() {
        let b = Buffer::from_str("abc");
        assert_eq!(b.cursor_to_char(Cursor::new(0, 3)).expect("0,3"), 3);
    }

    #[test]
    fn cursor_to_char_col_overflow() {
        let b = Buffer::from_str("ab");
        let err = b
            .cursor_to_char(Cursor::new(0, 5))
            .expect_err("cursor_to_char 应失败");
        assert!(matches!(err, Error::InvalidPosition { line: 0, col: 5 }));
    }

    #[test]
    fn cursor_to_char_line_overflow() {
        let b = Buffer::from_str("ab");
        let err = b
            .cursor_to_char(Cursor::new(5, 0))
            .expect_err("cursor_to_char 应失败");
        assert!(matches!(err, Error::InvalidPosition { line: 5, col: 0 }));
    }

    #[test]
    fn char_to_cursor_roundtrip() {
        let b = Buffer::from_str("abc\nde\nf");
        for cursor in [
            Cursor::new(0, 0),
            Cursor::new(0, 3),
            Cursor::new(1, 0),
            Cursor::new(1, 2),
            Cursor::new(2, 0),
            Cursor::new(2, 1),
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
    fn replace_reversed_range() {
        let mut b = Buffer::from_str("abcdef");
        let old = b
            .replace(Cursor::new(0, 5), Cursor::new(0, 2), "X")
            .expect("replace");
        assert_eq!(old, "cde");
        assert_eq!(b.to_string(), "abXf");
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
