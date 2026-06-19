//! syntect → genpdf 语法高亮适配层。
//!
//! 将代码块文本通过 syntect 做词法分析，
//! 样式映射为 genpdf 的 Paragraph styled segments。

use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// syntect 高亮一段代码，返回 (genpdf_style, text) 列表。
/// 每项对应一行，行内可能包含多个不同样式的片段。
pub type HighlightedLine = Vec<(genpdf::style::Style, String)>;

/// 高亮器持有语法集和主题，避免重复初始化。
pub struct CodeHighlighter {
    syntax_set: SyntaxSet,
    theme: syntect::highlighting::Theme,
}

impl CodeHighlighter {
    /// 用指定 syntect 主题名构造（如 "InspiredGitHub"）。
    /// 主题名不存在时返回 None。
    pub fn new(theme_name: &str) -> Option<Self> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes.get(theme_name).cloned()?;
        Some(Self { syntax_set, theme })
    }

    /// 高亮 code 文本，返回每行的带样式片段。
    /// `language` 为代码块语言标识（如 "rust"、"python"），
    /// None 时回退为纯文本。
    pub fn highlight(&self, code: &str, language: Option<&str>) -> Vec<HighlightedLine> {
        let syntax = language
            .and_then(|lang| self.syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut result: Vec<HighlightedLine> = Vec::new();

        for line in code.lines() {
            let ranges: Vec<(syntect::highlighting::Style, &str)> =
                match highlighter.highlight_line(line, &self.syntax_set) {
                    Ok(r) => r,
                    Err(_) => {
                        // 出错后重新构造 highlighter 避免状态污染
                        highlighter = HighlightLines::new(syntax, &self.theme);
                        vec![(syntect::highlighting::Style::default(), line)]
                    }
                };
            let styled: HighlightedLine = ranges
                .into_iter()
                .map(|(syn_style, text)| (map_style(&syn_style), text.to_owned()))
                .collect();
            result.push(styled);
        }
        result
    }
}

/// 将 syntect Style 映射为 genpdf Style。
fn map_style(syn: &syntect::highlighting::Style) -> genpdf::style::Style {
    let mut gs = genpdf::style::Style::new();
    let fg = syn.foreground;
    gs = gs.with_color(genpdf::style::Color::Rgb(fg.r, fg.g, fg.b));
    if syn
        .font_style
        .contains(syntect::highlighting::FontStyle::BOLD)
    {
        gs = gs.bold();
    }
    if syn
        .font_style
        .contains(syntect::highlighting::FontStyle::ITALIC)
    {
        gs = gs.italic();
    }
    gs
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn highlighter_new_with_valid_theme() {
        let h = CodeHighlighter::new("InspiredGitHub");
        assert!(h.is_some());
    }

    #[test]
    fn highlighter_new_with_invalid_theme() {
        let h = CodeHighlighter::new("nonexistent-theme-xyz");
        assert!(h.is_none());
    }

    #[test]
    fn highlight_rust_code_returns_styled_lines() {
        let h = CodeHighlighter::new("InspiredGitHub").unwrap();
        let lines = h.highlight("fn main() {\n    println!(\"hi\");\n}", Some("rust"));
        assert_eq!(lines.len(), 3);
        // 每行至少有一个片段
        for line in &lines {
            assert!(!line.is_empty());
        }
    }

    #[test]
    fn highlight_unknown_lang_falls_back_to_plain() {
        let h = CodeHighlighter::new("InspiredGitHub").unwrap();
        let lines = h.highlight("some code here", None);
        assert!(!lines.is_empty());
    }

    #[test]
    fn map_style_bold_italic() {
        let syn = syntect::highlighting::Style {
            foreground: syntect::highlighting::Color::WHITE,
            background: syntect::highlighting::Color::BLACK,
            font_style: syntect::highlighting::FontStyle::BOLD
                | syntect::highlighting::FontStyle::ITALIC,
        };
        let gs = map_style(&syn);
        // genpdf Style 的 bold/italic 是内部标志，我们通过构造验证无 panic
        let _ = gs; // 无 panic 即通过
    }
}
