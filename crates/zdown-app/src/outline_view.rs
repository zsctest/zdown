//! 大纲/目录面板：从 Document AST 提取标题，侧边栏渲染。

use document_model::ast::{Block, Document, Inline};
use editor_engine::Cursor;
use eframe::egui;

use crate::editor_state::EditorState;

/// 大纲项：标题层级、纯文本内容、源码行号。
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineItem {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// 将行内节点转换为纯文本（去除内联标记）。
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
    text
}

/// 从 Document AST 提取所有标题项。
pub fn extract_outline(doc: &Document) -> Vec<OutlineItem> {
    doc.blocks
        .iter()
        .filter_map(|bws| match &bws.block {
            Block::Heading(h) => {
                let text = inlines_to_plain(&h.inlines);
                let text = if text.is_empty() {
                    "(空标题)".to_string()
                } else {
                    text
                };
                Some(OutlineItem {
                    level: h.level,
                    text,
                    line: bws.span.start_line,
                })
            }
            _ => None,
        })
        .collect()
}

/// 渲染大纲侧边栏面板。
pub fn show_outline_panel(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.heading("📑 大纲");

    let doc = state.current_doc();
    let items = extract_outline(&doc);

    if items.is_empty() {
        ui.label(egui::RichText::new("（无标题）").weak());
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("outline_scroll")
        .show(ui, |ui| {
            for item in &items {
                let indent = (item.level.saturating_sub(1) as f32) * 16.0;
                ui.horizontal(|ui| {
                    ui.add_space(indent);

                    let text = if item.level <= 2 {
                        egui::RichText::new(&item.text).size(14.0).strong()
                    } else {
                        egui::RichText::new(&item.text).size(13.0).weak()
                    };

                    let response = ui.selectable_label(false, text);
                    if response.clicked() {
                        let cursor = Cursor::new(item.line, 0);
                        let _ = state.editor.set_cursor(cursor);
                    }
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use document_model::ast::{BlockWithSpan, Heading, Span};

    fn doc_from_blocks(blocks: Vec<BlockWithSpan>) -> Document {
        Document { blocks }
    }

    fn bws(block: Block, start_line: usize, end_line: usize) -> BlockWithSpan {
        BlockWithSpan {
            block,
            span: Span {
                start_line,
                end_line,
            },
        }
    }

    #[test]
    fn extract_outline_empty_doc() {
        let doc = doc_from_blocks(vec![]);
        assert_eq!(extract_outline(&doc), vec![]);
    }

    #[test]
    fn extract_outline_single_h1() {
        let doc = doc_from_blocks(vec![bws(
            Block::Heading(Heading {
                level: 1,
                inlines: vec![Inline::Text("简介".into())],
            }),
            0,
            0,
        )]);
        let items = extract_outline(&doc);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].level, 1);
        assert_eq!(items[0].text, "简介");
        assert_eq!(items[0].line, 0);
    }

    #[test]
    fn extract_outline_multiple_levels() {
        let doc = doc_from_blocks(vec![
            bws(
                Block::Heading(Heading {
                    level: 1,
                    inlines: vec![Inline::Text("第一章".into())],
                }),
                0,
                0,
            ),
            bws(
                Block::Heading(Heading {
                    level: 2,
                    inlines: vec![Inline::Text("第一节".into())],
                }),
                2,
                2,
            ),
            bws(
                Block::Heading(Heading {
                    level: 3,
                    inlines: vec![Inline::Text("小节".into())],
                }),
                5,
                5,
            ),
        ]);
        let items = extract_outline(&doc);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].level, 1);
        assert_eq!(items[1].level, 2);
        assert_eq!(items[2].level, 3);
        assert_eq!(items[2].line, 5);
    }

    #[test]
    fn extract_outline_no_headings() {
        let doc = doc_from_blocks(vec![bws(
            Block::Paragraph(document_model::ast::Paragraph {
                inlines: vec![Inline::Text("hello".into())],
            }),
            0,
            0,
        )]);
        assert_eq!(extract_outline(&doc), vec![]);
    }

    #[test]
    fn extract_outline_text_is_plain() {
        let doc = doc_from_blocks(vec![bws(
            Block::Heading(Heading {
                level: 1,
                inlines: vec![
                    Inline::Strong(vec![Inline::Text("重要".into())]),
                    Inline::Text("：".into()),
                    Inline::Link {
                        text: vec![Inline::Text("参考".into())],
                        url: "http://x".into(),
                        title: None,
                    },
                ],
            }),
            0,
            0,
        )]);
        let items = extract_outline(&doc);
        assert_eq!(items[0].text, "重要：参考");
    }

    #[test]
    fn extract_outline_empty_heading_text() {
        let doc = doc_from_blocks(vec![bws(
            Block::Heading(Heading {
                level: 2,
                inlines: vec![],
            }),
            3,
            3,
        )]);
        let items = extract_outline(&doc);
        assert_eq!(items[0].text, "(空标题)");
    }

    #[test]
    fn extract_outline_skips_non_headings() {
        let doc = doc_from_blocks(vec![
            bws(
                Block::Paragraph(document_model::ast::Paragraph {
                    inlines: vec![Inline::Text("text".into())],
                }),
                0,
                0,
            ),
            bws(
                Block::Heading(Heading {
                    level: 1,
                    inlines: vec![Inline::Text("标题".into())],
                }),
                1,
                1,
            ),
            bws(Block::ThematicBreak, 2, 2),
            bws(
                Block::Heading(Heading {
                    level: 2,
                    inlines: vec![Inline::Text("副标题".into())],
                }),
                3,
                3,
            ),
        ]);
        let items = extract_outline(&doc);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "标题");
        assert_eq!(items[1].text, "副标题");
    }
}
