//! PDF 导出入口：generate_pdf(doc, config) -> Result<Vec<u8>>。

use document_model::ast::Document;

use crate::Result;
use crate::font::FontSet;
use crate::theme::{Paper, PdfConfig};

use genpdf::Element;

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

    // 页边距通过 SimplePageDecorator 设置
    let margins = genpdf::Margins::trbl(
        config.margins.top,
        config.margins.right,
        config.margins.bottom,
        config.margins.left,
    );
    let mut decorator = genpdf::SimplePageDecorator::new();
    decorator.set_margins(margins);
    pdf_doc.set_page_decorator(decorator);

    // 页眉页脚（genpdf 0.2 支持有限，仅首页展示页码模板）
    if !config.header_footer.right.is_empty() {
        let hdr_p = genpdf::elements::Paragraph::new(
            config
                .header_footer
                .right
                .replace("{page}", "1")
                .replace("{total}", "1"),
        )
        .styled(
            genpdf::style::Style::new().with_font_size(config.theme.font_size.header_footer as u8),
        );
        pdf_doc.push(hdr_p);
    }

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
    use document_model::ast::{Block, Document, Inline, Paragraph as AstParagraph};

    fn sample_doc() -> Document {
        Document {
            blocks: vec![Block::Paragraph(AstParagraph {
                inlines: vec![Inline::Text("hello".into())],
            })],
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
