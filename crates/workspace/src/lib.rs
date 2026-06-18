//! workspace：文件 IO 与项目管理。
//!
//! 对外暴露 Workspace / pick_open_file / pick_save_file / RecentFiles / Error。
//! 实际职责见 docs/ARCHITECTURE.md §2.5。

pub mod dialog;
pub mod error;
pub mod recent;
pub mod workspace;

pub use dialog::{pick_open_file, pick_save_file};
pub use error::Error;
pub use workspace::Workspace;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "workspace");
    }
}
