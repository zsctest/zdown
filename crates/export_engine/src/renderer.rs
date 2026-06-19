//! AST -> genpdf element dispatch.
//!
//! `render_block` converts each `Block` variant into genpdf elements;
//! `render_inline_to_paragraph` applies per-inline styling
//! (emph, strong, code, link, etc.) into a Paragraph.

use image::GenericImageView;

use genpdf::Element;
use genpdf::elements::{Break, FrameCellDecorator, LinearLayout, Paragraph, TableLayout};
use genpdf::style::{Color, Style};

use crate::Result;
use crate::font::FontSet;
use crate::theme::{PdfConfig, PdfTheme};

use document_model::ast::{
    Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph as AstParagraph,
    Table,
};

/// 行内元素分段：按 Image 边界切分。
enum InlineSegment {
    /// 纯文本/样式的连续行内元素。
    Text(Vec<Inline>),
    /// 图片。
    Image {
        alt: String,
        url: String,
        /// 图片 title（暂未使用，保留供后续扩展）。
        _title: Option<String>,
    },
}

/// 将 inlines 列表按 `Inline::Image` 边界切分为 Text/Image 段。
fn split_inlines(inlines: &[Inline]) -> Vec<InlineSegment> {
    let mut segments: Vec<InlineSegment> = Vec::new();
    let mut current_text: Vec<Inline> = Vec::new();

    for inline in inlines {
        match inline {
            Inline::Image { alt, url, title } => {
                // 如果有累积的文本，先 push
                if !current_text.is_empty() {
                    segments.push(InlineSegment::Text(std::mem::take(&mut current_text)));
                }
                segments.push(InlineSegment::Image {
                    alt: alt.clone(),
                    url: url.clone(),
                    _title: title.clone(),
                });
            }
            other => {
                current_text.push(other.clone());
            }
        }
    }

    // 不要漏掉末尾的文本
    if !current_text.is_empty() {
        segments.push(InlineSegment::Text(current_text));
    } else if segments.is_empty() {
        // 全空列表 → 一个空 Text 段（保持返回至少一段）
        segments.push(InlineSegment::Text(Vec::new()));
    }

    segments
}

/// 根据图片像素尺寸和页面最大宽度计算缩放比例。
///
/// 原则：不放大（scale ≤ 1.0），只缩小超宽图片。
/// DPI 使用 printpdf 默认值 300。
fn auto_fit_scale(px_width: u32, _px_height: u32, max_width_mm: f64) -> genpdf::Scale {
    let dpi: f64 = 300.0;
    let mmpi: f64 = 25.4; // mm per inch
    let img_width_mm = (px_width as f64 / dpi) * mmpi;

    if img_width_mm <= max_width_mm {
        genpdf::Scale::new(1.0, 1.0)
    } else {
        let ratio = max_width_mm / img_width_mm;
        genpdf::Scale::new(ratio, ratio)
    }
}

/// Render a complete `Document` into a vertical genpdf layout.
pub fn render_document(
    doc: &Document,
    config: &PdfConfig,
    _fonts: &FontSet,
) -> Result<LinearLayout> {
    let mut layout = LinearLayout::vertical();
    let working_dir = config.working_dir.as_deref();
    let paper_width = match config.paper {
        crate::theme::Paper::A4 => 210.0,
        crate::theme::Paper::Letter => 215.9,
        crate::theme::Paper::Custom { width_mm, .. } => width_mm,
    };
    let max_width_mm =
        (paper_width as f64) - (config.margins.left as f64) - (config.margins.right as f64);
    for (i, bws) in doc.blocks.iter().enumerate() {
        if i > 0 {
            let gap = config.theme.spacing.paragraph_gap;
            if gap > 0.0 {
                layout.push(Break::new(1));
            }
        }
        render_block(
            &bws.block,
            &config.theme,
            working_dir,
            max_width_mm,
            &mut layout,
        )?;
    }
    Ok(layout)
}

fn render_block(
    block: &Block,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) -> Result<()> {
    match block {
        Block::Heading(h) => {
            render_heading(h, theme, working_dir, max_width_mm, layout);
        }
        Block::Paragraph(p) => {
            render_paragraph(p, theme, working_dir, max_width_mm, layout);
        }
        Block::CodeBlock(cb) => render_code_block(cb, theme, layout),
        Block::List(l) => render_list(
            l.ordered,
            l.start,
            &l.items,
            0,
            theme,
            working_dir,
            max_width_mm,
            layout,
        ),
        Block::BlockQuote(bq) => {
            render_blockquote(bq, theme, working_dir, max_width_mm, layout);
        }
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

/// 将 inlines 按 Image 分段后渲染到 layout。
/// Text 段 -> Paragraph，Image 段 -> elements::Image。
fn render_inlines_as_elements(
    inlines: &[Inline],
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    font_size: f32,
    layout: &mut LinearLayout,
) {
    let segments = split_inlines(inlines);

    let body_style = Style::new().with_font_size(font_size as u8);

    for segment in segments {
        match segment {
            InlineSegment::Text(text_inlines) => {
                let mut p = Paragraph::default();
                for inline in &text_inlines {
                    render_inline_to_paragraph(&mut p, inline, theme, font_size);
                }
                layout.push(p);
            }
            InlineSegment::Image { alt, url, .. } => {
                match crate::image_loader::load_image(&url, working_dir) {
                    Ok(dyn_img) => {
                        let (w, h) = dyn_img.dimensions();
                        let scale = auto_fit_scale(w, h, max_width_mm);
                        match genpdf::elements::Image::from_dynamic_image(dyn_img) {
                            Ok(img) => {
                                layout.push(
                                    img.with_alignment(genpdf::Alignment::Center)
                                        .with_scale(scale),
                                );
                            }
                            Err(_) => {
                                // 降级：占位文本
                                let mut p = Paragraph::default();
                                p.push_styled(format!("[图片: {alt}]"), body_style);
                                layout.push(p);
                            }
                        }
                    }
                    Err(_) => {
                        // 降级：占位文本
                        let mut p = Paragraph::default();
                        p.push_styled(format!("[图片: {alt}]"), body_style);
                        layout.push(p);
                    }
                }
            }
        }
    }
}

fn render_heading(
    h: &Heading,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    let font_size = match h.level {
        1 => theme.font_size.h1,
        2 => theme.font_size.h2,
        3 => theme.font_size.h3,
        4 => theme.font_size.h4,
        5 => theme.font_size.h5,
        _ => theme.font_size.h6,
    };
    render_inlines_as_elements(
        &h.inlines,
        theme,
        working_dir,
        max_width_mm,
        font_size,
        layout,
    );
}

fn render_paragraph(
    para: &AstParagraph,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    render_inlines_as_elements(
        &para.inlines,
        theme,
        working_dir,
        max_width_mm,
        theme.font_size.body,
        layout,
    );
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
            p.push_styled(format!("{} ({})", inlines_to_plain(text), url), link_style);
        }
        Inline::Image { alt, .. } => {
            p.push_styled(format!("[图片: {alt}]"), body_style);
        }
        Inline::Html(s) => p.push_styled(s.as_str(), body_style),
        Inline::SoftBreak => p.push_styled(" ", body_style),
        Inline::HardBreak => p.push("\n"),
    }
}

fn render_code_block(cb: &CodeBlock, theme: &PdfTheme, layout: &mut LinearLayout) {
    let highlighter = crate::highlight::CodeHighlighter::new(&theme.syntax_theme);
    let highlighted = highlighter
        .as_ref()
        .map(|h| h.highlight(&cb.content, cb.language.as_deref()));

    if let Some(lines) = highlighted {
        // 高亮版本：逐行逐 token 渲染
        let mut inner = LinearLayout::vertical();
        let code_font_size = theme.font_size.code as u8;
        for line in &lines {
            let mut p = Paragraph::default();
            if line.is_empty() {
                p.push_styled(" ", Style::new().with_font_size(code_font_size));
            } else {
                for (style, text) in line {
                    p.push_styled(text.as_str(), style.with_font_size(code_font_size));
                }
            }
            inner.push(p);
        }
        layout.push(inner.padded((4, 4, 4, 4)).framed());
    } else {
        // 回退：纯文本渲染
        let mut inner = LinearLayout::vertical();
        for line in cb.content.lines() {
            inner.push(
                Paragraph::new(line.to_owned())
                    .styled(Style::new().with_font_size(theme.font_size.code as u8)),
            );
        }
        layout.push(inner.padded((4, 4, 4, 4)).framed());
    }
}

fn render_blockquote(
    bq: &BlockQuote,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    let mut inner = LinearLayout::vertical();
    for bws in &bq.blocks {
        let _ = render_block(&bws.block, theme, working_dir, max_width_mm, &mut inner);
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

#[allow(clippy::too_many_arguments)]
fn render_list(
    ordered: bool,
    start: usize,
    items: &[ListItem],
    depth: usize,
    theme: &PdfTheme,
    working_dir: Option<&std::path::Path>,
    max_width_mm: f64,
    layout: &mut LinearLayout,
) {
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}. ", start + i)
        } else {
            "• ".to_owned()
        };
        let indent = theme.spacing.list_indent * (depth as f32);

        let segments = split_inlines(&item.inlines);

        for (seg_idx, segment) in segments.into_iter().enumerate() {
            match segment {
                InlineSegment::Text(text_inlines) => {
                    let mut p = Paragraph::default();
                    // 首个 Text 段添加 marker
                    if seg_idx == 0 {
                        p.push_styled(
                            &marker,
                            Style::new().with_font_size(theme.font_size.body as u8),
                        );
                    }
                    for inline in &text_inlines {
                        render_inline_to_paragraph(&mut p, inline, theme, theme.font_size.body);
                    }
                    layout.push(p.padded((0, 0, 0, indent)));
                }
                InlineSegment::Image { alt, url, .. } => {
                    match crate::image_loader::load_image(&url, working_dir) {
                        Ok(dyn_img) => {
                            let (w, h) = dyn_img.dimensions();
                            let scale = auto_fit_scale(w, h, max_width_mm);
                            match genpdf::elements::Image::from_dynamic_image(dyn_img) {
                                Ok(img) => {
                                    layout.push(
                                        img.with_alignment(genpdf::Alignment::Center)
                                            .with_scale(scale)
                                            .padded((0, 0, 0, indent)),
                                    );
                                }
                                Err(_) => {
                                    let mut p = Paragraph::default();
                                    p.push_styled(
                                        format!("[图片: {alt}]"),
                                        Style::new().with_font_size(theme.font_size.body as u8),
                                    );
                                    layout.push(p.padded((0, 0, 0, indent)));
                                }
                            }
                        }
                        Err(_) => {
                            let mut p = Paragraph::default();
                            p.push_styled(
                                format!("[图片: {alt}]"),
                                Style::new().with_font_size(theme.font_size.body as u8),
                            );
                            layout.push(p.padded((0, 0, 0, indent)));
                        }
                    }
                }
            }
        }

        if !item.sub_items.is_empty() {
            render_list(
                ordered,
                start,
                &item.sub_items,
                depth + 1,
                theme,
                working_dir,
                max_width_mm,
                layout,
            );
        }
    }
}

fn render_table(t: &Table, theme: &PdfTheme, layout: &mut LinearLayout) {
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

#[cfg(test)]
mod split_tests {
    use super::*;
    use document_model::ast::Inline;

    fn text(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    fn image(alt: &str, url: &str) -> Inline {
        Inline::Image {
            alt: alt.into(),
            url: url.into(),
            title: None,
        }
    }

    #[test]
    fn split_inlines_no_image() {
        let inlines = vec![text("hello"), text(" world")];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 1, "all text -> single Text segment");
        match &segments[0] {
            InlineSegment::Text(t) => assert_eq!(t.len(), 2),
            _ => panic!("expected Text segment"),
        }
    }

    #[test]
    fn split_inlines_single_image() {
        let inlines = vec![image("alt", "test.png")];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            InlineSegment::Image { alt, url, .. } => {
                assert_eq!(alt, "alt");
                assert_eq!(url, "test.png");
            }
            _ => panic!("expected Image segment"),
        }
    }

    #[test]
    fn split_inlines_mixed() {
        let inlines = vec![text("before "), image("mid", "mid.png"), text(" after")];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 3, "text + image + text = 3 segments");
        match (&segments[0], &segments[1], &segments[2]) {
            (InlineSegment::Text(_), InlineSegment::Image { .. }, InlineSegment::Text(_)) => {}
            _ => panic!("expected Text, Image, Text order"),
        }
    }

    #[test]
    fn split_inlines_consecutive_images() {
        let inlines = vec![image("a", "a.png"), image("b", "b.png"), text("trailing")];
        let segments = split_inlines(&inlines);
        assert_eq!(segments.len(), 3, "img + img + text = 3 segments");
    }

    #[test]
    fn split_inlines_empty() {
        let segments = split_inlines(&[]);
        assert_eq!(segments.len(), 1);
        match &segments[0] {
            InlineSegment::Text(t) => assert!(t.is_empty()),
            _ => panic!("expected empty Text segment"),
        }
    }

    #[test]
    fn auto_fit_scale_small_image() {
        let scale = auto_fit_scale(100, 100, 159.2);
        assert!(
            (scale.x - 1.0).abs() < 0.001,
            "small image x scale should be 1.0: got {}",
            scale.x
        );
        assert!(
            (scale.y - 1.0).abs() < 0.001,
            "small image y scale should be 1.0: got {}",
            scale.y
        );
    }

    #[test]
    fn auto_fit_scale_large_image() {
        // 600px @300dpi ≈ 50.8mm, max_width=25.4mm → scale ≈ 0.5
        let scale = auto_fit_scale(600, 300, 25.4);
        assert!(
            scale.x < 1.0,
            "large image should be scaled down, got {}",
            scale.x
        );
        assert!(
            (scale.x - scale.y).abs() < 0.001,
            "x and y scale should be equal"
        );
    }
}
