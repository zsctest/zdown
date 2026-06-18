//! editor_engine：文本编辑引擎。
//!
//! 对外暴露 Buffer / Cursor / Selection / Command / Editor / Error。
//! 实际职责见 docs/ARCHITECTURE.md §2.2。

pub mod buffer;
pub mod command;
pub mod cursor;
pub mod editor;
pub mod error;
pub mod history;

pub use buffer::Buffer;
pub use cursor::{Cursor, Selection};
pub use error::Error;
// TODO(任务 2): pub use command::{AppliedCommand, Command};
// TODO(任务 2): pub use history::History;
// TODO(任务 3): pub use editor::Editor;

/// crate 级 Result 别名。
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert_eq!(env!("CARGO_PKG_NAME"), "editor_engine");
    }
}
