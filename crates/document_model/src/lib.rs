//! document_model crate 占位骨架（阶段 0）
//!
//! 实际职责见 docs/ARCHITECTURE.md §2.1。AST 类型、解析、序列化在阶段 1 实施。

use thiserror::Error;

/// document_model 错误类型骨架。
///
/// 阶段 1 起按需扩展解析、序列化等错误变体。
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
        assert_eq!(env!("CARGO_PKG_NAME"), "document_model");
    }
}
