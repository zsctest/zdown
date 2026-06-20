//! Mermaid 图表渲染器。
//!
//! 将 Mermaid 语法通过 mermaid.ink 云端 API 渲染为 SVG。

#[allow(unused_imports)]
use std::time::Duration;

/// Mermaid 渲染错误。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("Mermaid 语法错误: {0}")]
    Syntax(String),
    #[error("HTTP 超时")]
    Timeout,
}

/// 渲染结果类型别名。
pub type Result<T> = std::result::Result<T, Error>;

/// Mermaid 图表渲染器。
pub struct MermaidRenderer;

impl Default for MermaidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidRenderer {
    /// 创建新渲染器。
    pub fn new() -> Self {
        Self
    }

    /// 判断 CodeBlock 是否为 mermaid 图表。
    pub fn is_mermaid(language: Option<&str>) -> bool {
        language.is_some_and(|l| l.eq_ignore_ascii_case("mermaid"))
    }

    /// 渲染 Mermaid 源码为 SVG 字符串（当前为桩实现）。
    pub fn render(&mut self, _source: &str) -> Result<String> {
        // 桩实现 — 后续任务会添加完整逻辑
        Err(Error::Network("renderer not yet connected".into()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn is_mermaid_detects_lowercase() {
        assert!(MermaidRenderer::is_mermaid(Some("mermaid")));
    }

    #[test]
    fn is_mermaid_case_insensitive() {
        assert!(MermaidRenderer::is_mermaid(Some("Mermaid")));
        assert!(MermaidRenderer::is_mermaid(Some("MERMAID")));
    }

    #[test]
    fn is_mermaid_rejects_other_languages() {
        assert!(!MermaidRenderer::is_mermaid(Some("rust")));
        assert!(!MermaidRenderer::is_mermaid(Some("python")));
        assert!(!MermaidRenderer::is_mermaid(None));
    }

    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "mermaid_renderer");
    }
}
