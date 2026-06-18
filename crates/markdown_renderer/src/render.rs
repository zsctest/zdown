//! AST → egui widget 渲染。
//!
//! 对外暴露 `render(ui, doc)`，按 Block/Inline 分发。
//! 注意：用 `egui`（非 `eframe::egui`），markdown_renderer 只依赖 egui crate。

use document_model::ast::{
    Alignment, Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph, Table,
    TableCell,
};

/// 将 `Document` 渲染到 egui UI。
pub fn render(ui: &mut egui::Ui, doc: &Document) {
    for block in &doc.blocks {
        render_block(ui, block);
    }
}

fn render_block(ui: &mut egui::Ui, block: &Block) {
    match block {
        Block::Heading(h) => render_heading(ui, h),
        Block::Paragraph(p) => render_paragraph(ui, p),
        Block::CodeBlock(cb) => render_code_block(ui, cb),
        Block::List(l) => render_list(ui, l.ordered, l.start, &l.items, 0),
        Block::BlockQuote(bq) => render_blockquote(ui, bq),
        Block::ThematicBreak => {
            ui.separator();
        }
        Block::Table(t) => render_table(ui, t),
        Block::HtmlBlock(s) => {
            ui.label(egui::RichText::new(s).code().weak());
        }
    }
}

fn render_heading(ui: &mut egui::Ui, h: &Heading) {
    // 标题用 inlines_to_richtext（接受 emph/strong 退化为纯文本，
    // 因 ui.heading 接受 RichText 不接受多个 label）
    let text = inlines_to_richtext(&h.inlines);
    let richtext = match h.level {
        1 => text.heading(),
        2 => text.size(24.0).strong(),
        3 => text.size(20.0).strong(),
        4 => text.size(18.0).strong(),
        5 => text.size(16.0).strong(),
        _ => text.size(14.0).strong(),
    };
    ui.heading(richtext);
}

fn render_paragraph(ui: &mut egui::Ui, p: &Paragraph) {
    // 任务 2 阶段先用 inlines_to_richtext，任务 3 改为 render_inlines
    ui.label(inlines_to_richtext(&p.inlines));
}

fn render_code_block(ui: &mut egui::Ui, cb: &CodeBlock) {
    let mut text = cb.content.clone();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .code_editor()
            .interactive(false)
            .desired_width(f32::INFINITY),
    );
}

/// 渲染列表。签名传 `&[ListItem]` 引用避免递归 clone（参考阶段 1 serialize.rs 修复）。
fn render_list(ui: &mut egui::Ui, ordered: bool, start: usize, items: &[ListItem], indent: usize) {
    ui.vertical(|ui| {
        for (i, item) in items.iter().enumerate() {
            let marker = if ordered {
                format!("{}. ", start + i)
            } else {
                "• ".to_owned()
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&marker).strong());
                ui.label(inlines_to_richtext(&item.inlines));
            });
            if !item.sub_items.is_empty() {
                ui.indent(egui::Id::new(format!("list_{indent}_{i}")), |ui| {
                    // 递归传 &item.sub_items（非父 List），避免无限递归
                    render_list(ui, ordered, start, &item.sub_items, indent + 1);
                });
            }
        }
    });
}

fn render_blockquote(ui: &mut egui::Ui, bq: &BlockQuote) {
    // 注意：egui 0.34 的 Frame::group 签名可能是 Frame::group(style) 或 Frame::group(ui)。
    // 若编译失败，按错误调整。
    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            for block in &bq.blocks {
                render_block(ui, block);
            }
        });
}

fn render_table(ui: &mut egui::Ui, t: &Table) {
    // 用指针地址生成唯一 id，避免同帧多表格冲突
    let table_id = egui::Id::new(format!("table_{:p}", t as *const Table));
    egui::Grid::new(table_id).striped(true).show(ui, |ui| {
        // 表头（应用对齐）
        for (col_idx, cell) in t.header.iter().enumerate() {
            let align = t.alignments.get(col_idx).copied().flatten();
            render_table_cell(ui, cell, align, true);
        }
        ui.end_row();
        // 数据行
        for row in &t.rows {
            for (col_idx, cell) in row.iter().enumerate() {
                let align = t.alignments.get(col_idx).copied().flatten();
                render_table_cell(ui, cell, align, false);
            }
            ui.end_row();
        }
    });
}

/// 渲染表格单元格，应用对齐。
///
/// egui 0.34 API：`RichText::into_layout_job` 为私有，改用
/// `WidgetText::into_galley`（公开）转换。再用 `Painter::galley` 绘制。
fn render_table_cell(
    ui: &mut egui::Ui,
    cell: &TableCell,
    align: Option<Alignment>,
    is_header: bool,
) {
    let richtext = inlines_to_richtext(&cell.inlines);
    let richtext = if is_header {
        richtext.strong()
    } else {
        richtext
    };
    let widget_text: egui::WidgetText = richtext.into();
    let galley = widget_text.into_galley(ui, None, f32::INFINITY, egui::FontSelection::Default);
    let (rect, response) = ui.allocate_at_least(galley.size(), egui::Sense::hover());
    let align_x = match align {
        Some(Alignment::Left) | None => egui::Align::LEFT,
        Some(Alignment::Center) => egui::Align::Center,
        Some(Alignment::Right) => egui::Align::RIGHT,
    };
    let pos = egui::Align2([align_x, egui::Align::TOP])
        .align_size_within_rect(galley.size(), rect)
        .min;
    ui.painter().galley(pos, galley, ui.visuals().text_color());
    let _ = response;
}

/// 将 Inline 列表转为 egui RichText（标题/表头用，emph/strong 退化为纯文本）。
fn inlines_to_richtext(inlines: &[Inline]) -> egui::RichText {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link {
                text: link_text, ..
            } => text.push_str(&inlines_to_plain(link_text)),
            Inline::Image { alt, .. } => {
                text.push_str("[图片: ");
                text.push_str(alt);
                text.push(']');
            }
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push('\n'),
            Inline::HardBreak => text.push('\n'),
        }
    }
    egui::RichText::new(text)
}

/// 将 Inline 列表转为纯文本（无样式）。
fn inlines_to_plain(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link {
                text: link_text, ..
            } => text.push_str(&inlines_to_plain(link_text)),
            Inline::Image { alt, .. } => text.push_str(alt),
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push('\n'),
            Inline::HardBreak => text.push('\n'),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn inlines_to_plain_text() {
        let inlines = vec![
            Inline::Text("hello".into()),
            Inline::SoftBreak,
            Inline::Text("world".into()),
        ];
        assert_eq!(inlines_to_plain(&inlines), "hello\nworld");
    }

    #[test]
    fn inlines_to_plain_emph_strong() {
        let inlines = vec![
            Inline::Emph(vec![Inline::Text("emph".into())]),
            Inline::Strong(vec![Inline::Text("strong".into())]),
        ];
        assert_eq!(inlines_to_plain(&inlines), "emphstrong");
    }

    #[test]
    fn inlines_to_plain_code() {
        let inlines = vec![Inline::Code("code".into())];
        assert_eq!(inlines_to_plain(&inlines), "code");
    }

    #[test]
    fn inlines_to_plain_link() {
        let inlines = vec![Inline::Link {
            text: vec![Inline::Text("text".into())],
            url: "https://x.com".into(),
            title: None,
        }];
        assert_eq!(inlines_to_plain(&inlines), "text");
    }

    #[test]
    fn inlines_to_plain_image() {
        let inlines = vec![Inline::Image {
            alt: "alt".into(),
            url: "https://x.com/x.png".into(),
            title: None,
        }];
        assert_eq!(inlines_to_plain(&inlines), "alt");
    }
}
