//! 往返测试：parse(src) → to_markdown → 规范化后应与规范化 src 等价。
//!
//! 规范化规则（与 serialize.rs 一致）：
//! - 标题统一 ATX
//! - 代码块统一 fenced
//! - 无序列表 marker 统一 `-`
//! - 连续空行 ≤ 1
//! - 行尾无空格
//! - 文档以单换行结尾

#![allow(clippy::expect_used)]

use document_model::{parse, to_markdown};
use std::fs;

fn fixture(name: &str) -> String {
    let path = format!("tests/fixtures/{name}.md");
    fs::read_to_string(path).expect("读取 fixture 失败")
}

/// 规范化 Markdown 源码，使往返比较稳定。
fn normalize(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut prev_blank = false;
    for line in src.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !prev_blank {
                out.push('\n');
            }
            prev_blank = true;
        } else {
            out.push_str(trimmed);
            out.push('\n');
            prev_blank = false;
        }
    }
    // 去除末尾多余空行，保留单换行结尾
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[test]
fn round_trip_heading() {
    let src = fixture("heading");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_paragraph() {
    let src = fixture("paragraph");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_code_block() {
    let src = fixture("code_block");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_list() {
    let src = fixture("list");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_nested_list() {
    let src = fixture("nested_list");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_blockquote() {
    let src = fixture("blockquote");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_table() {
    let src = fixture("table");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_link_image() {
    let src = fixture("link_image");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_mixed() {
    let src = fixture("mixed");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_commonmark_spec() {
    let src = fixture("commonmark_spec");
    let doc = parse(&src).expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(normalize(&out), normalize(&src));
}

#[test]
fn round_trip_empty() {
    let doc = parse("").expect("解析失败");
    let out = to_markdown(&doc);
    assert_eq!(out, "");
}

/// 反向：构造 AST → to_markdown → parse 应得等价 AST（忽略 span）。
#[test]
fn ast_to_markdown_back_to_ast() {
    use document_model::ast::*;
    use document_model::*;

    let original = Document {
        blocks: vec![
            BlockWithSpan {
                block: Block::Heading(Heading {
                    level: 1,
                    inlines: vec![Inline::Text("标题".into())],
                }),
                span: Span {
                    start_line: 0,
                    end_line: 0,
                },
            },
            BlockWithSpan {
                block: Block::Paragraph(Paragraph {
                    inlines: vec![Inline::Text("内容".into())],
                }),
                span: Span {
                    start_line: 0,
                    end_line: 0,
                },
            },
        ],
    };
    let md = to_markdown(&original);
    let reparsed = parse(&md).expect("重新解析失败");
    // 比较 block 内容（忽略 span，手工构造的 span 与 parse 填充的不同）
    let original_blocks: Vec<_> = original.blocks.into_iter().map(|bws| bws.block).collect();
    let reparsed_blocks: Vec<_> = reparsed.blocks.into_iter().map(|bws| bws.block).collect();
    assert_eq!(reparsed_blocks, original_blocks);
}
