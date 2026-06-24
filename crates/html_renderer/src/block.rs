//! 块级 HTML 标签 → egui Frame/Grid/Layout 渲染。

use egui::{Align, Layout, Ui};

use crate::css::{CssStyle, TextAlign};
use crate::inline;
use crate::parser::{BlockTag, HtmlNode};

/// 渲染块级 HtmlNode 列表。
pub(crate) fn render_block_nodes(ui: &mut Ui, nodes: &[HtmlNode]) {
    for node in nodes {
        render_block_node(ui, node);
    }
}

fn render_block_node(ui: &mut Ui, node: &HtmlNode) {
    match node {
        HtmlNode::Block {
            tag,
            style,
            children,
            ..
        } => match tag {
            BlockTag::Div | BlockTag::P | BlockTag::Blockquote => {
                render_div_like(ui, tag, style, children);
            }
            BlockTag::H1
            | BlockTag::H2
            | BlockTag::H3
            | BlockTag::H4
            | BlockTag::H5
            | BlockTag::H6 => {
                render_heading(ui, tag, style, children);
            }
            BlockTag::Pre => {
                render_pre(ui, style, children);
            }
            BlockTag::Hr => {
                ui.separator();
            }
            BlockTag::Table => {
                render_table(ui, style, children);
            }
            BlockTag::Ul => {
                render_list(ui, false, style, children);
            }
            BlockTag::Ol => {
                render_list(ui, true, style, children);
            }
            BlockTag::Li => {
                // <li> 出现在顶部时应渲染为段落
                render_div_like(ui, &BlockTag::Div, style, children);
            }
            BlockTag::Tr | BlockTag::Td | BlockTag::Th | BlockTag::Thead | BlockTag::Tbody => {
                // Table sub-elements at top level: render their text content
                let text: String = children
                    .iter()
                    .map(|c| match c {
                        HtmlNode::Text(s) => s.clone(),
                        _ => String::new(),
                    })
                    .collect();
                if !text.is_empty() {
                    ui.label(&text);
                }
            }
        },
        HtmlNode::Text(s) => {
            ui.label(s.as_str());
        }
        HtmlNode::Inline { .. } => {
            // 内联节点出现在块级上下文：用默认字体渲染
            let font_id = egui::FontId::default();
            inline::render_inline_nodes(ui, std::slice::from_ref(node), &font_id);
        }
    }
}

fn render_div_like(ui: &mut Ui, _tag: &BlockTag, style: &CssStyle, children: &[HtmlNode]) {
    let mut frame = egui::Frame::group(ui.style());

    // 应用 padding
    if let Some(pad) = style.padding {
        frame = frame.inner_margin(egui::Margin {
            left: pad.left as i8,
            right: pad.right as i8,
            top: pad.top as i8,
            bottom: pad.bottom as i8,
        });
    }

    // 应用 margin
    if let Some(m) = style.margin {
        frame = frame.outer_margin(egui::Margin {
            left: m.left as i8,
            right: m.right as i8,
            top: m.top as i8,
            bottom: m.bottom as i8,
        });
    }

    // 应用背景色
    if let Some(bg) = style.background_color {
        frame = frame.fill(bg);
    }

    let align_layout = text_align_to_layout(style.text_align);

    frame.show(ui, |ui| {
        ui.with_layout(align_layout, |ui| {
            for child in children {
                match child {
                    HtmlNode::Block { .. } => render_block_node(ui, child),
                    HtmlNode::Text(s) => {
                        ui.label(s.as_str());
                    }
                    HtmlNode::Inline { .. } => {
                        let font_id = egui::FontId::default();
                        inline::render_inline_nodes(ui, std::slice::from_ref(child), &font_id);
                    }
                }
            }
        });
    });
}

fn text_align_to_layout(align: Option<TextAlign>) -> Layout {
    match align {
        Some(TextAlign::Center) => Layout::top_down_justified(Align::Center),
        Some(TextAlign::Right) => Layout::right_to_left(Align::Min),
        _ => Layout::left_to_right(Align::Min),
    }
}

fn render_heading(ui: &mut Ui, tag: &BlockTag, style: &CssStyle, children: &[HtmlNode]) {
    let level = match tag {
        BlockTag::H1 => 1,
        BlockTag::H2 => 2,
        BlockTag::H3 => 3,
        BlockTag::H4 => 4,
        BlockTag::H5 => 5,
        BlockTag::H6 => 6,
        _ => 3,
    };

    let font_size = match level {
        1 => 28.0,
        2 => 24.0,
        3 => 20.0,
        4 => 18.0,
        5 => 16.0,
        _ => 14.0,
    };

    let heading_font = egui::FontId::new(font_size, egui::FontFamily::Proportional);
    let text: String = children
        .iter()
        .map(|c| match c {
            HtmlNode::Text(s) => s.clone(),
            _ => String::new(),
        })
        .collect();

    let mut rt = egui::RichText::new(text).strong().font(heading_font);

    if let Some(c) = style.color {
        rt = rt.color(c);
    }

    ui.label(rt);
}

fn render_pre(ui: &mut Ui, style: &CssStyle, children: &[HtmlNode]) {
    let text: String = children
        .iter()
        .map(|c| match c {
            HtmlNode::Text(s) => s.clone(),
            _ => String::new(),
        })
        .collect();

    let mut frame = egui::Frame::group(ui.style());

    if let Some(bg) = style.background_color {
        frame = frame.fill(bg);
    }

    frame.show(ui, |ui| {
        ui.label(
            egui::RichText::new(text)
                .monospace()
                .font(egui::FontId::monospace(13.0)),
        );
    });
}

fn render_table(ui: &mut Ui, _style: &CssStyle, children: &[HtmlNode]) {
    let rows = extract_table_rows(children);

    if rows.is_empty() {
        return;
    }

    let table_id = egui::Id::new(format!("html_table_{:p}", children.as_ptr()));
    egui::Grid::new(table_id).striped(true).show(ui, |ui| {
        for row in &rows {
            for cell in row {
                ui.label(cell);
            }
            ui.end_row();
        }
    });
}

fn extract_table_rows(nodes: &[HtmlNode]) -> Vec<Vec<String>> {
    let mut rows = vec![];

    for node in nodes {
        if let HtmlNode::Block { tag, children, .. } = node {
            match tag {
                BlockTag::Tr => {
                    let mut row = vec![];
                    for child in children {
                        match child {
                            HtmlNode::Block { tag, children, .. }
                                if *tag == BlockTag::Td || *tag == BlockTag::Th =>
                            {
                                let text: String = children
                                    .iter()
                                    .map(|c| match c {
                                        HtmlNode::Text(s) => s.clone(),
                                        _ => String::new(),
                                    })
                                    .collect();
                                row.push(text);
                            }
                            HtmlNode::Text(s) => {
                                row.push(s.clone());
                            }
                            _ => {}
                        }
                    }
                    if !row.is_empty() {
                        rows.push(row);
                    }
                }
                BlockTag::Thead | BlockTag::Tbody => {
                    rows.extend(extract_table_rows(children));
                }
                _ => {}
            }
        }
    }

    rows
}

fn render_list(ui: &mut Ui, ordered: bool, _style: &CssStyle, children: &[HtmlNode]) {
    ui.vertical(|ui| {
        let mut idx = 1usize;
        for node in children {
            match node {
                HtmlNode::Block { tag, children, .. } if *tag == BlockTag::Li => {
                    let marker = if ordered {
                        let m = format!("{idx}. ");
                        idx += 1;
                        m
                    } else {
                        "• ".to_owned()
                    };
                    let text: String = children
                        .iter()
                        .map(|c| match c {
                            HtmlNode::Text(s) => s.clone(),
                            _ => String::new(),
                        })
                        .collect();
                    ui.label(format!("{marker}{text}"));
                }
                HtmlNode::Text(s) => {
                    ui.label(s.as_str());
                }
                _ => {
                    render_block_node(ui, node);
                }
            }
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn extract_table_rows_basic() {
        let nodes = vec![HtmlNode::Block {
            tag: BlockTag::Tr,
            attrs: Default::default(),
            style: Default::default(),
            children: vec![
                HtmlNode::Block {
                    tag: BlockTag::Td,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("A".into())],
                },
                HtmlNode::Block {
                    tag: BlockTag::Td,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("B".into())],
                },
            ],
        }];
        let rows = extract_table_rows(&nodes);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[0][0], "A");
        assert_eq!(rows[0][1], "B");
    }

    #[test]
    fn extract_table_rows_with_thead() {
        let nodes = vec![HtmlNode::Block {
            tag: BlockTag::Thead,
            attrs: Default::default(),
            style: Default::default(),
            children: vec![HtmlNode::Block {
                tag: BlockTag::Tr,
                attrs: Default::default(),
                style: Default::default(),
                children: vec![HtmlNode::Block {
                    tag: BlockTag::Th,
                    attrs: Default::default(),
                    style: Default::default(),
                    children: vec![HtmlNode::Text("Header".into())],
                }],
            }],
        }];
        let rows = extract_table_rows(&nodes);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "Header");
    }

    #[test]
    fn extract_table_rows_empty() {
        let rows = extract_table_rows(&[]);
        assert!(rows.is_empty());
    }
}
