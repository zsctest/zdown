//! Markdown 序列化：AST → 规范化 Markdown 源码。
//!
//! 规范化规则（与往返测试配套）：
//! - 标题统一 ATX（`# ` 前缀），不用 setext
//! - 代码块统一 fenced（```），不用缩进
//! - 无序列表 marker 统一 `-`
//! - 连续空行 ≤ 1
//! - 行尾无空格
//! - 文档以单换行结尾

use crate::ast::{
    Alignment, Block, BlockQuote, CodeBlock, Document, Heading, Inline, ListItem, Paragraph, Table,
};

/// 将 `Document` 序列化为 Markdown 源码。
pub fn to_markdown(doc: &Document) -> String {
    let mut out = String::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        write_block(&mut out, block);
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn write_block(out: &mut String, block: &Block) {
    match block {
        Block::Heading(h) => write_heading(out, h),
        Block::Paragraph(p) => write_paragraph(out, p),
        Block::CodeBlock(cb) => write_code_block(out, cb),
        Block::List(l) => write_list(out, l.ordered, l.start, &l.items, 0),
        Block::BlockQuote(bq) => write_blockquote(out, bq),
        Block::ThematicBreak => {
            out.push_str("---\n");
        }
        Block::Table(t) => write_table(out, t),
        Block::HtmlBlock(s) => {
            out.push_str(s);
            if !s.ends_with('\n') {
                out.push('\n');
            }
        }
    }
}

fn write_heading(out: &mut String, h: &Heading) {
    for _ in 0..h.level {
        out.push('#');
    }
    out.push(' ');
    write_inlines(out, &h.inlines);
    out.push('\n');
}

fn write_paragraph(out: &mut String, p: &Paragraph) {
    write_inlines(out, &p.inlines);
    out.push('\n');
}

fn write_code_block(out: &mut String, cb: &CodeBlock) {
    out.push_str("```");
    if let Some(lang) = &cb.language {
        out.push_str(lang);
    }
    out.push('\n');
    out.push_str(&cb.content);
    if !cb.content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("```\n");
}

fn write_list(out: &mut String, ordered: bool, start: usize, items: &[ListItem], indent: usize) {
    for (i, item) in items.iter().enumerate() {
        let prefix: String = if ordered {
            format!("{}. ", start + i)
        } else {
            "- ".to_owned()
        };
        let prefix_len = prefix.len();
        write_indent(out, indent);
        out.push_str(&prefix);
        write_inlines(out, &item.inlines);
        out.push('\n');
        if !item.sub_items.is_empty() {
            // 子列表缩进 = 父级 prefix 长度（CommonMark 要求）
            write_list(out, ordered, start, &item.sub_items, indent + prefix_len);
        }
    }
}

fn write_blockquote(out: &mut String, bq: &BlockQuote) {
    let mut inner = String::new();
    for (i, block) in bq.blocks.iter().enumerate() {
        if i > 0 {
            inner.push('\n');
        }
        write_block(&mut inner, block);
    }
    for line in inner.lines() {
        out.push_str("> ");
        out.push_str(line);
        out.push('\n');
    }
}

fn write_table(out: &mut String, t: &Table) {
    // header
    out.push('|');
    for cell in &t.header {
        out.push(' ');
        write_inlines(out, &cell.inlines);
        out.push_str(" |");
    }
    out.push('\n');

    // 分隔行（含对齐）
    out.push('|');
    for align in &t.alignments {
        let sep = match align {
            Some(Alignment::Left) => ":---",
            Some(Alignment::Center) => ":---:",
            Some(Alignment::Right) => "---:",
            None => "---",
        };
        out.push(' ');
        out.push_str(sep);
        out.push_str(" |");
    }
    out.push('\n');

    // 数据行
    for row in &t.rows {
        out.push('|');
        for cell in row {
            out.push(' ');
            write_inlines(out, &cell.inlines);
            out.push_str(" |");
        }
        out.push('\n');
    }
}

fn write_inlines(out: &mut String, inlines: &[Inline]) {
    for inline in inlines {
        write_inline(out, inline);
    }
}

fn write_inline(out: &mut String, inline: &Inline) {
    match inline {
        Inline::Text(s) => out.push_str(s),
        Inline::Emph(inner) => {
            out.push('*');
            write_inlines(out, inner);
            out.push('*');
        }
        Inline::Strong(inner) => {
            out.push_str("**");
            write_inlines(out, inner);
            out.push_str("**");
        }
        Inline::Code(s) => {
            out.push('`');
            out.push_str(s);
            out.push('`');
        }
        Inline::Link { text, url, title } => {
            out.push('[');
            write_inlines(out, text);
            out.push_str("](");
            out.push_str(url);
            if let Some(t) = title {
                out.push_str(" \"");
                out.push_str(t);
                out.push('"');
            }
            out.push(')');
        }
        Inline::Image { alt, url, title } => {
            out.push_str("![");
            out.push_str(alt);
            out.push_str("](");
            out.push_str(url);
            if let Some(t) = title {
                out.push_str(" \"");
                out.push_str(t);
                out.push('"');
            }
            out.push(')');
        }
        Inline::Html(s) => out.push_str(s),
        Inline::SoftBreak => out.push('\n'),
        Inline::HardBreak => out.push_str("\\\n"),
    }
}

fn write_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::ast::{List, TableCell};

    #[test]
    fn empty_doc_to_empty_string() {
        let doc = Document { blocks: vec![] };
        assert_eq!(to_markdown(&doc), "");
    }

    #[test]
    fn heading_h1() {
        let doc = Document {
            blocks: vec![Block::Heading(Heading {
                level: 1,
                inlines: vec![Inline::Text("标题".into())],
            })],
        };
        assert_eq!(to_markdown(&doc), "# 标题\n");
    }

    #[test]
    fn heading_h3() {
        let doc = Document {
            blocks: vec![Block::Heading(Heading {
                level: 3,
                inlines: vec![Inline::Text("三".into())],
            })],
        };
        assert_eq!(to_markdown(&doc), "### 三\n");
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Text("hello".into())],
            })],
        };
        assert_eq!(to_markdown(&doc), "hello\n");
    }

    #[test]
    fn code_block_with_lang() {
        let doc = Document {
            blocks: vec![Block::CodeBlock(CodeBlock {
                language: Some("rust".into()),
                content: "fn x() {}\n".into(),
            })],
        };
        assert_eq!(to_markdown(&doc), "```rust\nfn x() {}\n```\n");
    }

    #[test]
    fn code_block_no_lang() {
        let doc = Document {
            blocks: vec![Block::CodeBlock(CodeBlock {
                language: None,
                content: "plain\n".into(),
            })],
        };
        assert_eq!(to_markdown(&doc), "```\nplain\n```\n");
    }

    #[test]
    fn unordered_list() {
        let doc = Document {
            blocks: vec![Block::List(List {
                ordered: false,
                start: 0,
                items: vec![
                    ListItem {
                        inlines: vec![Inline::Text("a".into())],
                        sub_items: vec![],
                    },
                    ListItem {
                        inlines: vec![Inline::Text("b".into())],
                        sub_items: vec![],
                    },
                ],
            })],
        };
        assert_eq!(to_markdown(&doc), "- a\n- b\n");
    }

    #[test]
    fn ordered_list_start_1() {
        let doc = Document {
            blocks: vec![Block::List(List {
                ordered: true,
                start: 1,
                items: vec![
                    ListItem {
                        inlines: vec![Inline::Text("一".into())],
                        sub_items: vec![],
                    },
                    ListItem {
                        inlines: vec![Inline::Text("二".into())],
                        sub_items: vec![],
                    },
                ],
            })],
        };
        assert_eq!(to_markdown(&doc), "1. 一\n2. 二\n");
    }

    #[test]
    fn nested_list() {
        let doc = Document {
            blocks: vec![Block::List(List {
                ordered: false,
                start: 0,
                items: vec![ListItem {
                    inlines: vec![Inline::Text("顶".into())],
                    sub_items: vec![ListItem {
                        inlines: vec![Inline::Text("嵌".into())],
                        sub_items: vec![],
                    }],
                }],
            })],
        };
        assert_eq!(to_markdown(&doc), "- 顶\n  - 嵌\n");
    }

    #[test]
    fn blockquote() {
        let doc = Document {
            blocks: vec![Block::BlockQuote(BlockQuote {
                blocks: vec![Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text("引用".into())],
                })],
            })],
        };
        assert_eq!(to_markdown(&doc), "> 引用\n");
    }

    #[test]
    fn thematic_break() {
        let doc = Document {
            blocks: vec![Block::ThematicBreak],
        };
        assert_eq!(to_markdown(&doc), "---\n");
    }

    #[test]
    fn emph() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Emph(vec![Inline::Text("e".into())])],
            })],
        };
        assert_eq!(to_markdown(&doc), "*e*\n");
    }

    #[test]
    fn strong() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Strong(vec![Inline::Text("s".into())])],
            })],
        };
        assert_eq!(to_markdown(&doc), "**s**\n");
    }

    #[test]
    fn inline_code() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Code("c".into())],
            })],
        };
        assert_eq!(to_markdown(&doc), "`c`\n");
    }

    #[test]
    fn link() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Link {
                    text: vec![Inline::Text("t".into())],
                    url: "https://x.com".into(),
                    title: None,
                }],
            })],
        };
        assert_eq!(to_markdown(&doc), "[t](https://x.com)\n");
    }

    #[test]
    fn link_with_title() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Link {
                    text: vec![Inline::Text("t".into())],
                    url: "https://x.com".into(),
                    title: Some("T".into()),
                }],
            })],
        };
        assert_eq!(to_markdown(&doc), "[t](https://x.com \"T\")\n");
    }

    #[test]
    fn image() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![Inline::Image {
                    alt: "a".into(),
                    url: "https://x.com/y.png".into(),
                    title: None,
                }],
            })],
        };
        assert_eq!(to_markdown(&doc), "![a](https://x.com/y.png)\n");
    }

    #[test]
    fn soft_break() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![
                    Inline::Text("a".into()),
                    Inline::SoftBreak,
                    Inline::Text("b".into()),
                ],
            })],
        };
        assert_eq!(to_markdown(&doc), "a\nb\n");
    }

    #[test]
    fn hard_break() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                inlines: vec![
                    Inline::Text("a".into()),
                    Inline::HardBreak,
                    Inline::Text("b".into()),
                ],
            })],
        };
        assert_eq!(to_markdown(&doc), "a\\\nb\n");
    }

    #[test]
    fn table() {
        let doc = Document {
            blocks: vec![Block::Table(Table {
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
            })],
        };
        assert_eq!(to_markdown(&doc), "| a | b |\n| --- | --- |\n| 1 | 2 |\n");
    }

    #[test]
    fn table_with_alignment() {
        let doc = Document {
            blocks: vec![Block::Table(Table {
                header: vec![
                    TableCell {
                        inlines: vec![Inline::Text("a".into())],
                    },
                    TableCell {
                        inlines: vec![Inline::Text("b".into())],
                    },
                ],
                rows: vec![],
                alignments: vec![Some(Alignment::Left), Some(Alignment::Center)],
            })],
        };
        assert_eq!(to_markdown(&doc), "| a | b |\n| :--- | :---: |\n");
    }

    #[test]
    fn multiple_blocks_separated_by_blank_line() {
        let doc = Document {
            blocks: vec![
                Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text("a".into())],
                }),
                Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text("b".into())],
                }),
            ],
        };
        assert_eq!(to_markdown(&doc), "a\n\nb\n");
    }

    #[test]
    fn nested_ordered_list_indent_follows_prefix_len() {
        // 有序列表嵌套：子项缩进 = 父级 prefix 长度（3 for "1. "）
        // 当前实现：子列表继承父级 ordered/start，故子项 marker 也是 "1. "
        let doc = Document {
            blocks: vec![Block::List(List {
                ordered: true,
                start: 1,
                items: vec![ListItem {
                    inlines: vec![Inline::Text("顶".into())],
                    sub_items: vec![ListItem {
                        inlines: vec![Inline::Text("嵌".into())],
                        sub_items: vec![],
                    }],
                }],
            })],
        };
        assert_eq!(to_markdown(&doc), "1. 顶\n   1. 嵌\n");
    }

    #[test]
    fn nested_ordered_list_start_10_indent_4() {
        // 父级 start=10 → "10. " 长度 4，子项缩进应为 4
        // 当前实现：子列表继承父级 ordered/start，故子项 marker 也是 "10. "
        let doc = Document {
            blocks: vec![Block::List(List {
                ordered: true,
                start: 10,
                items: vec![ListItem {
                    inlines: vec![Inline::Text("顶".into())],
                    sub_items: vec![ListItem {
                        inlines: vec![Inline::Text("嵌".into())],
                        sub_items: vec![],
                    }],
                }],
            })],
        };
        assert_eq!(to_markdown(&doc), "10. 顶\n    10. 嵌\n");
    }
}
