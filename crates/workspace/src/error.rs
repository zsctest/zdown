//! workspace 错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// 文件 IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    /// document_model 错误（解析或序列化）。
    #[error("文档错误: {0}")]
    Document(#[from] document_model::Error),
    /// 文件对话框错误（平台 API 不可用等）。
    #[error("对话框错误: {0}")]
    Dialog(String),
    /// TOML 反序列化错误。
    #[error("TOML 反序列化错误: {0}")]
    TomlDe(#[from] toml::de::Error),
    /// TOML 序列化错误。
    #[error("TOML 序列化错误: {0}")]
    TomlSer(#[from] toml::ser::Error),
    /// 当前路径未设置（save 无路径时）。
    #[error("未设置当前路径")]
    NoCurrentPath,
}
