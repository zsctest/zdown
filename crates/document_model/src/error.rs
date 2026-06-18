//! Error 类型（任务 4 中扩展变体）。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("功能尚未实现（阶段 0 占位）")]
    NotImplemented,
}
