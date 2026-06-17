//! markdown_renderer crate 占位骨架（阶段 0）
//!
//! 实际职责见 docs/ARCHITECTURE.md §2.3。AST → egui 组件渲染在阶段 2 实施。

use thiserror::Error;

/// markdown_renderer 错误类型骨架。
///
/// 阶段 2 起按需扩展渲染、缓存等错误变体。
#[derive(Debug, Error)]
pub enum Error {
    /// 占位变体：对应功能在后续阶段实施。
    #[error("功能尚未实现（阶段 0 占位）")]
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn error_display() {
        let err = Error::NotImplemented;
        assert_eq!(err.to_string(), "功能尚未实现（阶段 0 占位）");
    }

    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "markdown_renderer");
    }
}
