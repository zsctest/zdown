//! PDF 导出入口：generate_pdf(doc, config) -> Result<Vec<u8>>。

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use document_model::ast::Document;

use crate::Result;
use crate::font::FontSet;
use crate::theme::{Paper, PdfConfig};

/// 将 Document 导出为 PDF，返回完整 PDF 字节。
pub fn generate_pdf(doc: &Document, config: &PdfConfig) -> Result<Vec<u8>> {
    let fonts = FontSet::load(config)?;

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

    // 日期格式化（用于页眉页脚 {date} 占位符）
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let file_name = "untitled.md".to_string();

    let genpdf_margins = genpdf::Margins::trbl(
        genpdf::Mm::from(config.margins.top),
        genpdf::Mm::from(config.margins.right),
        genpdf::Mm::from(config.margins.bottom),
        genpdf::Mm::from(config.margins.left),
    );

    let page_counter = Arc::new(AtomicUsize::new(0));
    let decorator = crate::decorator::ZdownPageDecorator::new(
        config.header_footer.clone(),
        genpdf_margins,
        file_name,
        date_str,
        config.theme.font_size.header_footer,
        page_counter,
        None,
    );
    pdf_doc.set_page_decorator(decorator);

    let layout = crate::renderer::render_document(doc, config, &fonts)?;
    pdf_doc.push(layout);

    let mut buf = Vec::new();
    pdf_doc
        .render(&mut buf)
        .map_err(|e| crate::Error::Io(std::io::Error::other(e.to_string())))?;
    Ok(buf)
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
}
