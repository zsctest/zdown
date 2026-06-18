//! markdown_renderer 错误类型。
//!
//! 阶段 2 起按需扩展渲染、缓存等错误变体。

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
