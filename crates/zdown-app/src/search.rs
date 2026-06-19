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
                let start_ok = col == 0 || !line_chars[col - 1].is_alphanumeric();
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
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn whole_word_with_underscore() {
        let m = SearchEngine::find_all("foo_bar foo", "foo", &opts_word());
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
