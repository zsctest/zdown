//! export_engine crate 占位骨架（阶段 0）
//!
//! 实际职责见 docs/ARCHITECTURE.md §2.4。HTML / PDF 导出在阶段 4 实施。

use thiserror::Error;

/// export_engine 错误类型骨架。
///
/// 阶段 4 起按需扩展 HTML / PDF 导出等错误变体。
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
        assert_eq!(env!("CARGO_PKG_NAME"), "export_engine");
    }
}
