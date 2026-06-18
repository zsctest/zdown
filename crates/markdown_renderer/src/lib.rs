//! markdown_renderer：AST → egui 组件渲染（阶段 2）+ 源码高亮（阶段 1）。

pub mod error;
pub mod render;
pub mod source;

pub use error::Error;
pub use render::{RenderCache, render};
pub use source::{SourceHighlighter, StyledLine};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "markdown_renderer");
    }
}
