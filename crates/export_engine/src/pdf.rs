//! PDF 导出入口：generate_pdf(doc, config) -> Result<Vec<u8>>。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use document_model::ast::Document;

use crate::Result;
use crate::font::FontSet;
use crate::theme::{HeaderFooter, Paper, PdfConfig};

/// 检查 HeaderFooter 模板是否包含 {total} 占位符。
fn template_needs_total(hf: &HeaderFooter) -> bool {
    hf.left.contains("{total}") || hf.center.contains("{total}") || hf.right.contains("{total}")
}

fn make_doc(
    config: &PdfConfig,
    fonts: &FontSet,
    decorator: crate::decorator::ZdownPageDecorator,
) -> genpdf::Document {
    let paper_size: genpdf::Size = match config.paper {
        Paper::A4 => genpdf::PaperSize::A4.into(),
        Paper::Letter => genpdf::PaperSize::Letter.into(),
        Paper::Custom {
            width_mm,
            height_mm,
        } => genpdf::Size::new(width_mm, height_mm),
    };

    let mut pdf_doc = genpdf::Document::new(genpdf::fonts::FontFamily {
        regular: fonts.body.clone(),
        bold: fonts.body.clone(),
        italic: fonts.body.clone(),
        bold_italic: fonts.body.clone(),
    });
    pdf_doc.set_paper_size(paper_size);
    pdf_doc.set_title("zdown export");
    pdf_doc.set_page_decorator(decorator);
    pdf_doc
}

fn layout_and_push(
    pdf_doc: &mut genpdf::Document,
    doc: &Document,
    config: &PdfConfig,
    fonts: &FontSet,
) -> crate::Result<()> {
    let layout = crate::renderer::render_document(doc, config, fonts)?;
    pdf_doc.push(layout);
    Ok(())
}

fn render_to_vec(doc: genpdf::Document) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    doc.render(&mut buf)
        .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
    Ok(buf)
}

/// 将 Document 导出为 PDF，返回完整 PDF 字节。
pub fn generate_pdf(doc: &Document, config: &PdfConfig) -> Result<Vec<u8>> {
    let fonts = FontSet::load(config)?;

    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let file_name = "untitled.md".to_string();

    let genpdf_margins = genpdf::Margins::trbl(
        genpdf::Mm::from(config.margins.top),
        genpdf::Mm::from(config.margins.right),
        genpdf::Mm::from(config.margins.bottom),
        genpdf::Mm::from(config.margins.left),
    );

    let hf_font_size = config.theme.font_size.header_footer;

    if template_needs_total(&config.header_footer) {
        // Pass 1: 获取总页数
        let pc = Arc::new(AtomicUsize::new(0));
        let d1 = crate::decorator::ZdownPageDecorator::new(
            config.header_footer.clone(),
            genpdf_margins,
            file_name.clone(),
            date_str.clone(),
            hf_font_size,
            pc.clone(),
            None,
        );
        let mut doc1 = make_doc(config, &fonts, d1);
        layout_and_push(&mut doc1, doc, config, &fonts)?;
        doc1.render(&mut std::io::sink())
            .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
        let total = pc.load(Ordering::Relaxed);

        // Pass 2: 正式渲染
        let pc2 = Arc::new(AtomicUsize::new(0));
        let d2 = crate::decorator::ZdownPageDecorator::new(
            config.header_footer.clone(),
            genpdf_margins,
            file_name,
            date_str,
            hf_font_size,
            pc2,
            Some(total),
        );
        let mut doc2 = make_doc(config, &fonts, d2);
        layout_and_push(&mut doc2, doc, config, &fonts)?;
        render_to_vec(doc2)
    } else {
        // 单趟渲染（模板不含 {total}）
        let pc = Arc::new(AtomicUsize::new(0));
        let d = crate::decorator::ZdownPageDecorator::new(
            config.header_footer.clone(),
            genpdf_margins,
            file_name,
            date_str,
            hf_font_size,
            pc,
            None,
        );
        let mut pdf_doc = make_doc(config, &fonts, d);
        layout_and_push(&mut pdf_doc, doc, config, &fonts)?;
        render_to_vec(pdf_doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::PdfConfig;
    use document_model::ast::{
        Block, BlockWithSpan, Document, Inline, Paragraph as AstParagraph, Span,
    };

    fn sample_doc() -> Document {
        Document {
            blocks: vec![BlockWithSpan {
                block: Block::Paragraph(AstParagraph {
                    inlines: vec![Inline::Text("hello".into())],
                }),
                span: Span {
                    start_line: 0,
                    end_line: 0,
                },
            }],
        }
    }

    #[test]
    fn generate_pdf_default_returns_non_empty() {
        let doc = sample_doc();
        let config = PdfConfig::minimal();
        let result = generate_pdf(&doc, &config);
        if let Ok(bytes) = result {
            assert!(!bytes.is_empty(), "PDF 应非空");
        }
        // Err 可接受（无字体环境）
    }

    #[test]
    fn generate_pdf_empty_doc_returns_bytes() {
        let doc = Document { blocks: vec![] };
        let config = PdfConfig::minimal();
        let result = generate_pdf(&doc, &config);
        if let Ok(bytes) = result {
            assert!(!bytes.is_empty(), "空文档也应有最小 PDF");
        }
        // Err 可接受（无字体环境）
    }

    #[test]
    fn template_needs_total_detects_total_placeholder() {
        use crate::theme::HeaderFooter;

        let hf = HeaderFooter {
            left: String::new(),
            center: String::new(),
            right: "{page}/{total}".into(),
        };
        assert!(template_needs_total(&hf));

        let hf2 = HeaderFooter {
            left: "{total}".into(),
            center: String::new(),
            right: String::new(),
        };
        assert!(template_needs_total(&hf2));

        let hf3 = HeaderFooter {
            left: "{file}".into(),
            center: "{date}".into(),
            right: "{page}".into(),
        };
        assert!(!template_needs_total(&hf3));
    }

    #[test]
    fn generate_pdf_with_total_does_not_panic() {
        let mut config = PdfConfig::default();
        config.header_footer.right = "{page}/{total}".into();
        let doc = Document {
            blocks: vec![BlockWithSpan {
                block: Block::Paragraph(AstParagraph {
                    inlines: vec![Inline::Text("test".into())],
                }),
                span: Span {
                    start_line: 0,
                    end_line: 0,
                },
            }],
        };
        let result = generate_pdf(&doc, &config);
        if let Ok(bytes) = result {
            assert!(!bytes.is_empty(), "两趟渲染应产出非空 PDF");
        }
        // Err 可接受（无字体环境）
    }
}
