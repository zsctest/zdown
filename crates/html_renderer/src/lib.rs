//! HTML 内嵌渲染器。
//!
//! 将 Markdown 中的内嵌 HTML 解析为 DOM 树并渲染为 egui 富文本/块级组件。

pub mod block;
pub mod css;
pub mod inline;
pub mod parser;

use egui::{FontId, Ui};

/// 渲染内联 HTML 字符串（在段落内调用）。
///
/// 解析失败或标签未识别时回退为源码弱化文本显示。
pub fn render_inline_html(ui: &mut Ui, html: &str, base_font: &FontId) {
    let _ = ui;
    let _ = html;
    let _ = base_font;
}

/// 渲染块级 HTML 字符串（在 Block context 调用）。
pub fn render_block_html(ui: &mut Ui, html: &str) {
    let _ = ui;
    let _ = html;
}
