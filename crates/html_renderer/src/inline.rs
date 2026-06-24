//! 内联 HTML 标签 → egui RichText 渲染。

use std::collections::HashMap;

use egui::{FontId, RichText, Ui};

use crate::css::{CssStyle, FontStyle, FontWeight, TextDecoration};
use crate::parser::{HtmlNode, InlineTag};

/// 渲染内联 HtmlNode 列表。
pub(crate) fn render_inline_nodes(ui: &mut Ui, nodes: &[HtmlNode], base_font: &FontId) {
    ui.horizontal_wrapped(|ui| {
        for node in nodes {
            render_inline_node(ui, node, base_font);
        }
    });
}

fn render_inline_node(ui: &mut Ui, node: &HtmlNode, base_font: &FontId) {
    match node {
        HtmlNode::Text(s) => {
            ui.label(RichText::new(s.as_str()).font(base_font.clone()));
        }
        HtmlNode::Inline {
            tag,
            attrs,
            style,
            children,
        } => {
            // 嵌套子节点：累积子文本
            let child_text = children
                .iter()
                .map(collect_text)
                .collect::<Vec<_>>()
                .join("");
            let rt = build_richtext(tag, attrs, style, base_font, &child_text);

            // 根据标签类型渲染
            match tag {
                InlineTag::A => {
                    let url = attrs.get("href").map(|s| s.as_str()).unwrap_or("#");
                    ui.hyperlink_to(rt, url);
                }
                InlineTag::Br => {
                    ui.label(RichText::new("\n").font(base_font.clone()));
                }
                _ => {
                    ui.label(rt);
                }
            }
        }
        HtmlNode::Block { .. } => {
            // 块级节点不应出现在内联上下文中，忽略
        }
    }
}

fn build_richtext(
    tag: &InlineTag,
    _attrs: &HashMap<String, String>,
    style: &CssStyle,
    base_font: &FontId,
    text: &str,
) -> RichText {
    let mut rt = RichText::new(text);

    // 应用 CSS 样式
    if let Some(c) = style.color {
        rt = rt.color(c);
    }
    if let Some(c) = style.background_color {
        rt = rt.background_color(c);
    }
    if let Some(size) = style.font_size {
        rt = rt.font(FontId::new(size, base_font.family.clone()));
    } else {
        rt = rt.font(base_font.clone());
    }
    match style.font_weight {
        Some(FontWeight::Bold) => {
            rt = rt.strong();
        }
        Some(FontWeight::Normal) => {}
        None => {}
    }
    match style.font_style {
        Some(FontStyle::Italic) => {
            rt = rt.italics();
        }
        Some(FontStyle::Normal) => {}
        None => {}
    }
    match style.text_decoration {
        Some(TextDecoration::Underline) => {
            rt = rt.underline();
        }
        Some(TextDecoration::LineThrough) => {
            rt = rt.strikethrough();
        }
        None => {}
    }

    // 应用标签语义（如果 CSS 未覆盖）
    match tag {
        InlineTag::B => {
            if style.font_weight.is_none() {
                rt = rt.strong();
            }
        }
        InlineTag::I => {
            if style.font_style.is_none() {
                rt = rt.italics();
            }
        }
        InlineTag::U => {
            if style.text_decoration.is_none() {
                rt = rt.underline();
            }
        }
        InlineTag::Del => {
            if style.text_decoration.is_none() {
                rt = rt.strikethrough();
            }
        }
        InlineTag::Code => {
            rt = rt.code();
        }
        InlineTag::Mark => {
            if style.background_color.is_none() {
                rt = rt.background_color(egui::Color32::from_rgb(255, 255, 0));
            }
        }
        InlineTag::Sub => {
            let small_size = base_font.size * 0.75;
            rt = rt.font(FontId::new(small_size, base_font.family.clone()));
        }
        InlineTag::Sup => {
            let small_size = base_font.size * 0.75;
            rt = rt.font(FontId::new(small_size, base_font.family.clone()));
        }
        InlineTag::Small => {
            let small_size = base_font.size * 0.85;
            rt = rt.font(FontId::new(small_size, base_font.family.clone()));
        }
        InlineTag::Big => {
            let big_size = base_font.size * 1.2;
            rt = rt.font(FontId::new(big_size, base_font.family.clone()));
        }
        InlineTag::Span | InlineTag::Br | InlineTag::A => {}
    }

    rt
}

pub(crate) fn collect_text(node: &HtmlNode) -> String {
    match node {
        HtmlNode::Text(s) => s.clone(),
        HtmlNode::Inline { children, .. } | HtmlNode::Block { children, .. } => {
            children.iter().map(collect_text).collect()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn collect_text_simple() {
        let node = HtmlNode::Text("hello".into());
        assert_eq!(collect_text(&node), "hello");
    }

    #[test]
    fn collect_text_nested() {
        let node = HtmlNode::Inline {
            tag: InlineTag::B,
            attrs: Default::default(),
            style: Default::default(),
            children: vec![
                HtmlNode::Text("bold ".into()),
                HtmlNode::Inline {
                    tag: InlineTag::I,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("italic".into())],
                },
            ],
        };
        assert_eq!(collect_text(&node), "bold italic");
    }
}
