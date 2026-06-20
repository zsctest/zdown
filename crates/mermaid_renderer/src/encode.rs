//! Mermaid 源码 → mermaid.ink URL 编码。
//!
//! 编码流程：UTF-8 字节 → deflate 压缩 → base64url。

use base64::Engine;

/// 将 Mermaid 源码编码为 mermaid.ink GET URL。
pub fn encode_to_url(source: &str) -> String {
    let encoded = encode_mermaid(source);
    format!("https://mermaid.ink/img/pako:{encoded}")
}

/// 对 Mermaid 源码执行 pako deflate + base64url 编码。
fn encode_mermaid(source: &str) -> String {
    let input = source.as_bytes();
    let compressed = deflate(input);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
}

/// deflate 压缩（模拟 pako 行为）。
///
/// # Panics
///
/// 仅在底层 IO 出现不可恢复错误时 panic ；
/// 此处 `Vec<u8>` writer 的实现是 infallible 的，故不会 panic。
#[allow(clippy::expect_used)]
fn deflate(data: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).expect("deflate write");
    encoder.finish().expect("deflate finish")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn encode_simple_graph_produces_url() {
        let source = "graph TD\n    A --> B";
        let url = encode_to_url(source);
        assert!(url.starts_with("https://mermaid.ink/img/pako:"));
    }

    #[test]
    fn encode_empty_source_returns_url() {
        let url = encode_to_url("");
        assert!(url.starts_with("https://mermaid.ink/img/pako:"));
    }

    #[test]
    fn encode_preserves_unicode() {
        let source = "graph LR\n    开始 --> 结束";
        let url = encode_to_url(source);
        assert!(url.contains("pako:"));
    }

    #[test]
    fn encode_sequence_diagram() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello";
        let url = encode_to_url(source);
        assert!(url.starts_with("https://mermaid.ink/img/pako:"));
    }

    #[test]
    fn encode_produces_url_safe_output() {
        let source = "graph TD";
        let url = encode_to_url(source);
        // URL 安全编码不应包含 '+' 或 '/'
        let encoded = url.strip_prefix("https://mermaid.ink/img/pako:").unwrap();
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
    }

    #[test]
    fn same_input_produces_same_url() {
        let src = "graph TD\n    A-->B";
        let url1 = encode_to_url(src);
        let url2 = encode_to_url(src);
        assert_eq!(url1, url2);
    }
}
