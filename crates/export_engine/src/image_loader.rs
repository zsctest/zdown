//! 图片加载模块（stub — 将在任务 3 实现）。
//!
//! 负责从本地路径、base64 data URI 或网络 URL 加载图片。

use crate::Result;
use crate::error::Error;

/// 从路径加载图片到 RGBA 字节缓冲区。
pub fn load_image(_path: &str) -> Result<(Vec<u8>, u32, u32)> {
    Err(Error::ImageLoad("not implemented".into()))
}
