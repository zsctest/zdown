//! export_engine：Markdown → PDF/HTML 导出（阶段 3）。

pub mod decorator;
pub mod error;
pub mod font;
pub mod highlight;
pub mod html;
pub mod image_loader;
pub mod pdf;
pub mod renderer;
pub mod theme;

pub use error::Error;
pub use html::{HtmlConfig, generate_html};
pub use pdf::generate_pdf;
pub use theme::PdfConfig;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "export_engine");
    }
}
