//! 源码语法高亮（syntect）。
//!
//! 有状态高亮：一次处理全文，保留跨行语法状态（代码块、多行强调）。
//! 阶段 2 扩展为 AST → egui 组件渲染，本模块仅做源码行级高亮。

use std::io;

use syntect::LoadingError;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme};
use syntect::parsing::SyntaxSet;

/// 一行的高亮结果：样式片段列表。
pub type StyledLine<'a> = Vec<(Style, &'a str)>;

/// 源码高亮器。
pub struct SourceHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SourceHighlighter {
    /// 用默认语法集 + 默认主题（`base16-ocean.dark`）构造。
    pub fn new() -> Result<Self, LoadingError> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Ok(Self { syntax_set, theme })
    }

    /// 用指定主题名构造（如 `InspiredGitHub` / `base16-eighties.dark`）。
    ///
    /// 主题名不存在时返回 `LoadingError::Io(NotFound)`。
    /// 注：syntect 5.3 `LoadingError` 无 `InvalidTheme` 变体，借用 `Io` 表达。
    pub fn with_theme(theme_name: &str) -> Result<Self, LoadingError> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();
        let theme = theme_set.themes.get(theme_name).cloned().ok_or_else(|| {
            LoadingError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("theme not found: {theme_name}"),
            ))
        })?;
        Ok(Self { syntax_set, theme })
    }

    /// 高亮全文，返回每行的样式片段列表。
    ///
    /// `language` 为 None 时按 markdown 语法高亮；Some(lang) 时按指定语言。
    pub fn highlight<'a>(&self, src: &'a str, language: Option<&str>) -> Vec<StyledLine<'a>> {
        let syntax = match language {
            Some(lang) => self
                .syntax_set
                .find_syntax_by_token(lang)
                .or_else(|| self.syntax_set.find_syntax_by_extension(lang)),
            None => self.syntax_set.find_syntax_by_extension("md"),
        };
        let syntax = syntax.unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut result = Vec::new();
        for line in src.lines() {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    let styled: StyledLine = ranges.into_iter().collect();
                    result.push(styled);
                }
                Err(_) => {
                    result.push(vec![(Default::default(), line)]);
                }
            }
        }
        result
    }

    /// 主题引用（egui 转换颜色用）。
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn new_loads_defaults() {
        let h = SourceHighlighter::new().expect("load");
        // theme.settings.background 应为 Some 或 None（取决于主题），验证不 panic 即可
        let _ = &h.theme;
    }

    #[test]
    fn highlight_empty_returns_empty() {
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight("", None);
        assert!(result.is_empty());
    }

    #[test]
    fn highlight_single_line_returns_one_styled_line() {
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight("# 标题", None);
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_empty());
    }

    #[test]
    fn highlight_multiline_returns_per_line() {
        let src = "# 标题\n\n段落文本\n";
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(src, None);
        // lines() 忽略末尾换行，故 3 行
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn highlight_code_block_preserves_state() {
        let src = "```rust\nfn main() {}\nlet x = 1;\n```\n";
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(src, None);
        // 4 行：```rust / fn main() {} / let x = 1; / ```
        assert_eq!(result.len(), 4);
        // 代码块内行应有多个样式片段（rust 语法高亮）
        assert!(!result[1].is_empty(), "代码行应有样式: {:?}", result[1]);
    }

    #[test]
    fn highlight_with_language_rust() {
        let src = "fn main() { println!(\"hi\"); }";
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(src, Some("rust"));
        assert_eq!(result.len(), 1);
        // rust 语法应产生多个样式片段（fn / main / println 等）
        assert!(
            result[0].len() > 1,
            "rust 语法应分多个片段: {:?}",
            result[0]
        );
    }

    #[test]
    fn highlight_with_unknown_language_falls_back_to_plain() {
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight("hello", Some("nonexistent-lang"));
        // 不 panic，返回结果
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn styled_line_str_lifetime_matches_input() {
        let src = String::from("# 标题\n段落");
        let h = SourceHighlighter::new().expect("load");
        let result = h.highlight(&src, None);
        // 验证 &str 借用 src：合并所有片段应能还原原文（按行）
        let line0: String = result[0].iter().map(|(_, s)| *s).collect();
        assert_eq!(line0, "# 标题");
        let line1: String = result[1].iter().map(|(_, s)| *s).collect();
        assert_eq!(line1, "段落");
    }

    #[test]
    fn with_theme_inspired_github() {
        let h = SourceHighlighter::with_theme("InspiredGitHub").expect("theme");
        let _ = h.highlight("# t", None);
    }

    #[test]
    fn with_theme_invalid_returns_err() {
        let result = SourceHighlighter::with_theme("nonexistent-theme-xyz");
        assert!(result.is_err());
    }
}
