//! editor_engine 错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("无效位置: line {line}, col {col}")]
    InvalidPosition { line: usize, col: usize },
    #[error("无效范围")]
    InvalidRange,
    #[error("操作越界: {0}")]
    OutOfBounds(String),
}
