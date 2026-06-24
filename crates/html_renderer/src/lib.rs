//! HTML 内嵌渲染器。
//!
//! 将 Markdown 中的内嵌 HTML 解析为 DOM 树并渲染为 egui 富文本/块级组件。

pub mod block;
pub mod css;
pub mod inline;
pub mod parser;

use egui::{FontId, RichText, Ui};

/// 渲染内联 HTML 字符串（在段落内调用）。
///
/// 解析失败或标签未识别时回退为源码弱化文本显示。
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId) {
    let nodes = parser::parse_inline(html);
    if nodes.is_empty() {
        return;
    }
    // 如果解析结果只有 Text 节点且内容与原始 HTML 相同（解析失败回退），
    // 则显示为弱文本
    if nodes.len() == 1 {
        if let parser::HtmlNode::Text(ref s) = nodes[0] {
            if s == html {
                ui.label(RichText::new(html).weak().font(base_font.clone()));
                return;
            }
        }
    }
    inline::render_inline_nodes(ui, &nodes, base_font);
}

/// 渲染块级 HTML 字符串（在 Block context 调用）。
pub fn render_block_html(ui: &mut Ui, html: &str) {
    let nodes = parser::parse_block(html);
    if nodes.is_empty() {
        return;
    }
    block::render_block_nodes(ui, &nodes);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "html_renderer");
    }

    #[test]
    fn render_inline_html_empty() {
        // 空字符串不 panic
        let _ = parser::parse_inline("");
    }

    #[test]
    fn render_inline_html_bold() {
        let nodes = parser::parse_inline("<b>hello</b>");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn render_block_html_div() {
        let nodes = parser::parse_block("<div>content</div>");
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn render_inline_html_nested_mixed() {
        let nodes = parser::parse_inline("<b>bold <i>and italic</i></b> text");
        assert!(!nodes.is_empty());
    }

    #[test]
    fn render_block_html_table() {
        let html = "<table><tr><td>A</td><td>B</td></tr></table>";
        let nodes = parser::parse_block(html);
        assert!(!nodes.is_empty());
    }

    #[test]
    fn render_inline_html_br() {
        let nodes = parser::parse_inline("line1<br>line2");
        assert_eq!(nodes.len(), 3); // Text, Br, Text
    }
}
