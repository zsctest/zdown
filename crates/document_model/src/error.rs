//! document_model 错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// 解析 Markdown 源码失败。
    #[error("解析错误: {0}")]
    Parse(String),
    /// 序列化 AST 为 Markdown 失败。
    #[error("序列化错误: {0}")]
    Serialize(String),
}
