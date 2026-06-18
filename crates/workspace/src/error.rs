//! workspace 错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// 文件 IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    /// document_model 解析/序列化错误。
    #[error("文档错误: {0}")]
    Parse(#[from] document_model::Error),
    /// 文件对话框错误（平台 API 不可用等）。
    #[error("对话框错误: {0}")]
    Dialog(String),
    /// TOML 序列化/反序列化错误。
    #[error("TOML 错误: {0}")]
    Serialize(#[from] toml::de::Error),
    /// 当前路径未设置（save 无路径时）。
    #[error("未设置当前路径")]
    NoCurrentPath,
}
