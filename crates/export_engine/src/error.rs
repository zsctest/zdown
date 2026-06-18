//! export_engine 错误类型。

use thiserror::Error;

/// export_engine 错误。
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("字体加载失败: {0}")]
    FontLoad(String),

    #[error("PDF 渲染错误: {0}")]
    Render(String),
}
