//! AST -> genpdf element dispatch.
//!
//! `render_block` converts each `Block` variant into genpdf elements;
//! `render_inline_to_paragraph` applies per-inline styling
//! (emph, strong, code, link, etc.) into a Paragraph.

use genpdf::elements::{Break, FrameCellDecorator, LinearLayout, Paragraph, TableLayout};
use genpdf::style::{Color, Style};
use genpdf::Element;

use crate::font::FontSet;
use crate::theme::{PdfConfig, PdfTheme};
use crate::Result;

use document_model::ast::{
    Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph as AstParagraph,
    Table,
};

/// Render a complete `Document` into a vertical genpdf layout.
pub fn render_document(
    doc: &Document,
    config: &PdfConfig,
    _fonts: &FontSet,
) -> Result<LinearLayout> {
    let mut layout = LinearLayout::vertical();
    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            let gap = config.theme.spacing.paragraph_gap;
            if gap > 0.0 {
                layout.push(Break::new(1));
            }
        }
        render_block(block, &config.theme, &mut layout)?;
    }
    Ok(layout)
}

fn render_block(
    block: &Block,
    theme: &PdfTheme,
    layout: &mut LinearLayout,
) -> Result<()> {
    match block {
        Block::Heading(h) => layout.push(render_heading(h, theme)),
        Block::Paragraph(p) => layout.push(render_paragraph(p, theme)),
        Block::CodeBlock(cb) => render_code_block(cb, theme, layout),
        Block::List(l) => render_list(l.ordered, l.start, &l.items, 0, theme, layout),
        Block::BlockQuote(bq) => render_blockquote(bq, theme, layout),
        Block::ThematicBreak => {
            layout.push(
                Paragraph::new("─".repeat(60))
                    .aligned(genpdf::Alignment::Center)
                    .padded((0, 4)),
            );
        }
        Block::Table(t) => render_table(t, theme, layout),
        Block::HtmlBlock(_) => {}
    }
    Ok(())
}

fn render_heading(h: &Heading, theme: &PdfTheme) -> Paragraph {
    let font_size = match h.level {
        1 => theme.font_size.h1,
        2 => theme.font_size.h2,
        3 => theme.font_size.h3,
        4 => theme.font_size.h4,
        5 => theme.font_size.h5,
        _ => theme.font_size.h6,
    };
    let mut p = Paragraph::default();
    for inline in &h.inlines {
        render_inline_to_paragraph(&mut p, inline, theme, font_size);
    }
    p
}

fn render_paragraph(para: &AstParagraph, theme: &PdfTheme) -> Paragraph {
    let mut p = Paragraph::default();
    for inline in &para.inlines {
        render_inline_to_paragraph(&mut p, inline, theme, theme.font_size.body);
    }
    p
}

fn render_inline_to_paragraph(
    p: &mut Paragraph,
    inline: &Inline,
    theme: &PdfTheme,
    font_size: f32,
) {
    let body_style = Style::new().with_font_size(font_size as u8);
    let italic_style = body_style.italic();
    let bold_style = body_style.bold();
    let mono_style = Style::new().with_font_size(theme.font_size.code as u8);
    let link_style = Style::new()
        .with_color(Color::Rgb(0, 0, 255))
        .with_font_size(font_size as u8);

    match inline {
        Inline::Text(s) => p.push_styled(s.as_str(), body_style),
        Inline::Emph(inner) => {
            let text = inlines_to_plain(inner);
            p.push_styled(text, italic_style);
        }
        Inline::Strong(inner) => {
            let text = inlines_to_plain(inner);
            p.push_styled(text, bold_style);
        }
        Inline::Code(s) => p.push_styled(s.as_str(), mono_style),
        Inline::Link { text, url, .. } => {
            p.push_styled(
                format!("{} ({})", inlines_to_plain(text), url),
                link_style,
            );
        }
        Inline::Image { alt, .. } => {
            p.push_styled(format!("[图片: {alt}]"), body_style);
        }
        Inline::Html(s) => p.push_styled(s.as_str(), body_style),
        Inline::SoftBreak => p.push_styled(" ", body_style),
        Inline::HardBreak => p.push("\n"),
    }
}

fn render_code_block(
    cb: &CodeBlock,
    theme: &PdfTheme,
    layout: &mut LinearLayout,
) {
    let mut inner = LinearLayout::vertical();
    for line in cb.content.lines() {
        inner.push(
            Paragraph::new(line.to_owned())
                .styled(Style::new().with_font_size(theme.font_size.code as u8)),
        );
    }
    layout.push(inner.padded((4, 4, 4, 4)).framed());
}

fn render_blockquote(
    bq: &BlockQuote,
    theme: &PdfTheme,
    layout: &mut LinearLayout,
) {
    let mut inner = LinearLayout::vertical();
    for block in &bq.blocks {
        let _ = render_block(block, theme, &mut inner);
    }
    layout.push(inner.framed().padded((0, 0, 0, 4)));
}

/// Flatten an `Inline` slice into a plain `String`.
fn inlines_to_plain(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_plain(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link { text: lt, .. } => text.push_str(&inlines_to_plain(lt)),
            Inline::Image { alt, .. } => text.push_str(alt),
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push(' '),
            Inline::HardBreak => text.push(' '),
        }
    }
    text
}

fn render_list(
    ordered: bool,
    start: usize,
    items: &[ListItem],
    depth: usize,
    theme: &PdfTheme,
    layout: &mut LinearLayout,
) {
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}. ", start + i)
        } else {
            "• ".to_owned()
        };
        let indent = theme.spacing.list_indent * (depth as f32);

        let mut p = Paragraph::default();
        p.push_styled(&marker, Style::new().with_font_size(theme.font_size.body as u8));
        for inline in &item.inlines {
            render_inline_to_paragraph(&mut p, inline, theme, theme.font_size.body);
        }
        layout.push(p.padded((0, 0, 0, indent)));

        if !item.sub_items.is_empty() {
            render_list(ordered, start, &item.sub_items, depth + 1, theme, layout);
        }
    }
}

fn render_table(
    t: &Table,
    theme: &PdfTheme,
    layout: &mut LinearLayout,
) {
    let ncols = if !t.header.is_empty() {
        t.header.len()
    } else if let Some(row) = t.rows.first() {
        row.len()
    } else {
        return;
    };

    let mut table_layout = TableLayout::new(vec![1; ncols]);
    table_layout.set_cell_decorator(FrameCellDecorator::new(true, false, false));
    let padding = theme.spacing.cell_padding;

    // 表头行
    if !t.header.is_empty() {
        let header_style = Style::new()
            .bold()
            .with_font_size(theme.font_size.body as u8);
        let mut row = table_layout.row();
        for cell in &t.header {
            let text = inlines_to_richtext_str(&cell.inlines);
            row = row.element(Paragraph::new(text).styled(header_style).padded(padding));
        }
        let _ = row.push();
    }

    // 数据行
    let cell_style = Style::new().with_font_size(theme.font_size.body as u8);
    for row_data in &t.rows {
        let mut row = table_layout.row();
        for cell in row_data {
            let text = inlines_to_richtext_str(&cell.inlines);
            row = row.element(Paragraph::new(text).styled(cell_style).padded(padding));
        }
        let _ = row.push();
    }

    layout.push(table_layout);
}

/// 将 Inline 列表转为纯文本（用于表格单元格）。
fn inlines_to_richtext_str(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(s) => text.push_str(s),
            Inline::Emph(inner) => text.push_str(&inlines_to_richtext_str(inner)),
            Inline::Strong(inner) => text.push_str(&inlines_to_richtext_str(inner)),
            Inline::Code(s) => text.push_str(s),
            Inline::Link { text: lt, .. } => text.push_str(&inlines_to_richtext_str(lt)),
            Inline::Image { alt, .. } => {
                text.push_str("[图片: ");
                text.push_str(alt);
                text.push(']');
            }
            Inline::Html(s) => text.push_str(s),
            Inline::SoftBreak => text.push(' '),
            Inline::HardBreak => text.push(' '),
        }
    }
    text
}
