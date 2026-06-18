//! document_model：Markdown 文档数据模型。
//!
//! 对外暴露 `Document`、`parse`、`to_markdown`、`Error`。
//! 实际职责见 docs/ARCHITECTURE.md §2.1。

pub mod ast;
pub mod error;
pub mod parse;
pub mod serialize;

pub use ast::*;
pub use error::Error;
pub use parse::parse;
pub use serialize::to_markdown;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

impl Document {
    /// 序列化为 Markdown 源码（委托到 `to_markdown` 顶层函数）。
    pub fn to_markdown(&self) -> String {
        crate::to_markdown(self)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "document_model");
    }
}
