//! hybrid 模式性能测试：parse 缓存命中 < 50ms，增量 parse < 50ms。
//!
//! 运行：`cargo test --test hybrid_perf -- --ignored --nocapture`
//! 注：render 延迟需 egui Context，由手动验证。

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)]

use std::time::Instant;

fn generate_large_markdown(target_bytes: usize) -> String {
    let paragraph = "性能测试文本，含中文与 English。";
    let mut out = String::with_capacity(target_bytes);
    let mut idx = 0;
    while out.len() < target_bytes {
        out.push_str(&format!("# 标题 {idx}\n\n{paragraph}\n\n"));
        idx += 1;
    }
    out
}

#[test]
#[ignore = "性能测试，手动运行"]
fn hybrid_parse_cached_under_50ms() {
    let src = generate_large_markdown(500_000); // 500KB
    let mut cache = markdown_renderer::RenderCache::new();

    let start = Instant::now();
    let _doc = cache.parse_cached(&src);
    let cold = start.elapsed();
    println!("冷启动 parse 500KB: {cold:?}");

    let start = Instant::now();
    let _doc = cache.parse_cached(&src);
    let hot = start.elapsed();
    println!("缓存命中 parse 500KB: {hot:?}");

    assert!(hot.as_millis() < 50, "缓存命中应 < 50ms，实际 {hot:?}");
}

#[test]
#[ignore = "性能测试，手动运行"]
fn hybrid_incremental_parse_under_50ms() {
    let mut src = generate_large_markdown(100_000); // 100KB
    let mut cache = markdown_renderer::RenderCache::new();

    for i in 0..10 {
        src.push_str(&format!("编辑{i}\n"));
        let start = Instant::now();
        let _doc = cache.parse_cached(&src);
        let elapsed = start.elapsed();
        println!("增量编辑 #{i} parse: {elapsed:?}");
        assert!(
            elapsed.as_millis() < 50,
            "增量 parse 100KB 应 < 50ms，实际 {elapsed:?}"
        );
    }
}
