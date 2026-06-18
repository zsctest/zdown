//! 性能测试：大文件 parse + Buffer::from_str < 200ms。
//!
//! 仅测核心（parse + Buffer 构造），UI 渲染手动评估。
//! 运行：`cargo test -p document_model --test perf -- --ignored --nocapture`

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)]

use std::time::Instant;

/// 生成约 1MB Markdown 内容（含标题/段落/列表/代码块混合）。
fn generate_large_markdown(target_bytes: usize) -> String {
    let paragraph = "这是一段用于性能测试的 Markdown 文本，含中文与 English 混排。";
    let mut out = String::with_capacity(target_bytes);
    let mut idx = 0;
    while out.len() < target_bytes {
        out.push_str(&format!("# 标题 {idx}\n\n"));
        out.push_str(paragraph);
        out.push_str("\n\n");
        out.push_str("- 列表项一\n- 列表项二\n- 列表项三\n\n");
        out.push_str("```rust\nfn x() -> u32 { ");
        out.push_str(&idx.to_string());
        out.push_str(" }\n```\n\n");
        idx += 1;
    }
    out
}

#[test]
#[ignore = "性能测试，手动运行：cargo test --test perf -- --ignored --nocapture"]
fn parse_large_file_under_200ms() {
    let src = generate_large_markdown(1_000_000);
    assert!(
        src.len() >= 1_000_000,
        "生成内容应 ≥ 1MB，实际 {} bytes",
        src.len()
    );

    let start = Instant::now();
    let doc = document_model::parse(&src).expect("解析失败");
    let elapsed = start.elapsed();

    println!(
        "parse {} bytes ({} blocks) 耗时: {:?}",
        src.len(),
        doc.blocks.len(),
        elapsed
    );

    assert!(
        elapsed.as_millis() < 200,
        "parse 1MB 文件应 < 200ms，实际 {:?}",
        elapsed
    );
}

#[test]
#[ignore = "性能测试，手动运行：cargo test --test perf -- --ignored --nocapture"]
fn buffer_from_str_large_file_under_50ms() {
    let src = generate_large_markdown(1_000_000);

    let start = Instant::now();
    let buffer = editor_engine::Buffer::from_str(&src);
    let elapsed = start.elapsed();

    println!(
        "Buffer::from_str {} bytes ({} lines) 耗时: {:?}",
        src.len(),
        buffer.len_lines(),
        elapsed
    );

    assert!(
        elapsed.as_millis() < 50,
        "Buffer::from_str 1MB 文件应 < 50ms，实际 {:?}",
        elapsed
    );
}

#[test]
#[ignore = "性能测试，手动运行：cargo test --test perf -- --ignored --nocapture"]
fn parse_plus_buffer_under_200ms() {
    let src = generate_large_markdown(1_000_000);

    let start = Instant::now();
    let _doc = document_model::parse(&src).expect("解析失败");
    let _buffer = editor_engine::Buffer::from_str(&src);
    let elapsed = start.elapsed();

    println!(
        "parse + Buffer::from_str {} bytes 耗时: {:?}",
        src.len(),
        elapsed
    );

    assert!(
        elapsed.as_millis() < 200,
        "parse + Buffer::from_str 1MB 文件应 < 200ms，实际 {:?}",
        elapsed
    );
}
