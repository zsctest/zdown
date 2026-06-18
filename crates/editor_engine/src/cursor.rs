//! 光标与选区。
//!
//! Cursor 用 (line, col) 表示，col 为**字符列**（非字节列）。

/// 文本光标位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

impl Cursor {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub const fn zero() -> Self {
        Self::new(0, 0)
    }
}

/// 文本选区。`start` 是锚点，`end` 是活动端（光标所在）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    pub start: Cursor,
    pub end: Cursor,
}

impl Selection {
    pub const fn new(start: Cursor, end: Cursor) -> Self {
        Self { start, end }
    }

    pub const fn caret(pos: Cursor) -> Self {
        Self::new(pos, pos)
    }

    pub fn is_caret(&self) -> bool {
        self.start == self.end
    }

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
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
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
        let s = Selection::new(b, a);
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
