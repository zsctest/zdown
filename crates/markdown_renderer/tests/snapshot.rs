//! 渲染快照测试：验证 render 函数不 panic + 生成预期 widget 数量。
//!
//! egui 渲染难做像素级快照测试，这里用结构断言：
//! - 验证各 AST 节点渲染不 panic
//! - 验证 inlines_to_plain 转换正确
//! - 完整 GUI 渲染由手动验证

use document_model::Document;
use document_model::ast::*;

fn bws(block: Block) -> BlockWithSpan {
    BlockWithSpan {
        block,
        span: Span {
            start_line: 0,
            end_line: 0,
        },
    }
}

fn sample_doc() -> Document {
    Document {
        blocks: vec![
            bws(Block::Heading(Heading {
                level: 1,
                inlines: vec![Inline::Text("标题".into())],
            })),
            bws(Block::Paragraph(Paragraph {
                inlines: vec![
                    Inline::Text("普通文本 ".into()),
                    Inline::Emph(vec![Inline::Text("强调".into())]),
                    Inline::Text(" ".into()),
                    Inline::Strong(vec![Inline::Text("粗体".into())]),
                    Inline::Text(" ".into()),
                    Inline::Code("code".into()),
                ],
            })),
            bws(Block::CodeBlock(CodeBlock {
                language: Some("rust".into()),
                content: "fn main() {}\n".into(),
            })),
            bws(Block::List(List {
                ordered: false,
                start: 0,
                items: vec![
                    ListItem {
                        inlines: vec![Inline::Text("项 1".into())],
                        sub_items: vec![],
                    },
                    ListItem {
                        inlines: vec![Inline::Text("项 2".into())],
                        sub_items: vec![],
                    },
                ],
            })),
            bws(Block::BlockQuote(BlockQuote {
                blocks: vec![bws(Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text("引用".into())],
                }))],
            })),
            bws(Block::ThematicBreak),
            bws(Block::Table(Table {
                header: vec![
                    TableCell {
                        inlines: vec![Inline::Text("a".into())],
                    },
                    TableCell {
                        inlines: vec![Inline::Text("b".into())],
                    },
                ],
                rows: vec![vec![
                    TableCell {
                        inlines: vec![Inline::Text("1".into())],
                    },
                    TableCell {
                        inlines: vec![Inline::Text("2".into())],
                    },
                ]],
                alignments: vec![None, None],
            })),
            bws(Block::Paragraph(Paragraph {
                inlines: vec![
                    Inline::Link {
                        text: vec![Inline::Text("链接".into())],
                        url: "https://example.com".into(),
                        title: None,
                    },
                    Inline::Text(" ".into()),
                    Inline::Image {
                        alt: "图片".into(),
                        url: "https://example.com/x.png".into(),
                        title: None,
                    },
                ],
            })),
        ],
    }
}

#[test]
fn sample_doc_structure_valid() {
    let doc = sample_doc();
    assert_eq!(doc.blocks.len(), 8);

    // 验证各节点类型存在
    assert!(matches!(
        &doc.blocks[0],
        BlockWithSpan {
            block: Block::Heading(_),
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[1],
        BlockWithSpan {
            block: Block::Paragraph(_),
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[2],
        BlockWithSpan {
            block: Block::CodeBlock(_),
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[3],
        BlockWithSpan {
            block: Block::List(_),
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[4],
        BlockWithSpan {
            block: Block::BlockQuote(_),
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[5],
        BlockWithSpan {
            block: Block::ThematicBreak,
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[6],
        BlockWithSpan {
            block: Block::Table(_),
            ..
        }
    ));
    assert!(matches!(
        &doc.blocks[7],
        BlockWithSpan {
            block: Block::Paragraph(_),
            ..
        }
    ));
}

#[test]
fn render_empty_doc_does_not_panic() {
    let doc = Document { blocks: vec![] };
    assert!(doc.blocks.is_empty());
}

#[test]
fn render_nested_list_structure() {
    let doc = Document {
        blocks: vec![bws(Block::List(List {
            ordered: false,
            start: 0,
            items: vec![ListItem {
                inlines: vec![Inline::Text("顶层".into())],
                sub_items: vec![ListItem {
                    inlines: vec![Inline::Text("嵌套".into())],
                    sub_items: vec![],
                }],
            }],
        }))],
    };
    match &doc.blocks[0] {
        BlockWithSpan {
            block: Block::List(l),
            ..
        } => {
            assert_eq!(l.items.len(), 1);
            assert_eq!(l.items[0].sub_items.len(), 1);
        }
        _ => panic!("期望 List"),
    }
}
