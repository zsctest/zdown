//! 图片加载模块。
//!
//! 负责从本地路径、base64 data URI 或网络 URL 加载图片。

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use image::DynamicImage;
use image::GenericImageView;

use crate::Result;
use crate::error::Error;

/// 根据 URL 类型分发到对应的加载函数。
pub fn load_image(url: &str, working_dir: Option<&Path>) -> Result<DynamicImage> {
    if url.starts_with("data:") {
        load_from_data_uri(url)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        load_from_remote(url)
    } else {
        load_from_local(url, working_dir)
    }
}

/// 加载 data URI 图片（`data:image/<type>;base64,<data>`）。
fn load_from_data_uri(url: &str) -> Result<DynamicImage> {
    let b64: Vec<&str> = url.splitn(2, ";base64,").collect();
    if b64.len() < 2 {
        return Err(Error::ImageLoad("无效 data URI 格式：缺少 ;base64,".into()));
    }
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64[1])
        .map_err(|e| Error::ImageLoad(format!("base64 解码失败: {e}")))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| Error::ImageLoad(format!("图片解码失败: {e}")))?;
    Ok(flatten_alpha(img))
}

/// 加载远程 URL 图片。
fn load_from_remote(url: &str) -> Result<DynamicImage> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| Error::ImageLoad(format!("远程请求失败: {e}")))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| Error::ImageLoad(format!("读取远程响应失败: {e}")))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| Error::ImageLoad(format!("图片解码失败: {e}")))?;
    Ok(flatten_alpha(img))
}

/// 加载本地文件图片。
fn load_from_local(path_str: &str, working_dir: Option<&Path>) -> Result<DynamicImage> {
    let path = std::path::Path::new(path_str);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let wd = working_dir
            .ok_or_else(|| Error::ImageLoad("相对路径图片需要设置 working_dir".into()))?;
        wd.join(path_str)
    };
    let img =
        image::open(&resolved).map_err(|e| Error::ImageLoad(format!("打开本地图片失败: {e}")))?;
    Ok(flatten_alpha(img))
}

/// 将带 alpha 通道的图片展平到白色背景上，返回 RGB 无 alpha 的图片。
fn flatten_alpha(img: DynamicImage) -> DynamicImage {
    if img.color().has_alpha() {
        let (w, h) = img.dimensions();
        let mut bg = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut bg, &img.to_rgba8(), 0, 0);
        image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(bg).to_rgb8())
    } else {
        img
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    /// 测试 1：从有效的 data URI 加载 1x1 红色 PNG。
    #[test]
    fn load_from_data_uri_success() {
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let uri = format!("data:image/png;base64,{b64}");
        let img = match load_image(&uri, None) {
            Ok(img) => img,
            Err(e) => panic!("应从有效 data URI 加载图片，但失败: {e}"),
        };
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
    }

    /// 测试 2：无效 base64 返回错误。
    #[test]
    fn load_from_data_uri_invalid_base64() {
        let uri = "data:image/png;base64,!!!not-valid-base64!!!";
        let result = load_image(uri, None);
        match result {
            Err(e) => assert!(e.to_string().contains("base64 解码失败")),
            Ok(_) => panic!("无效 base64 应返回错误"),
        }
    }

    /// 测试 3：不存在的本地文件返回错误。
    #[test]
    fn load_from_local_nonexistent() {
        let nonexistent = std::env::temp_dir().join("__zdown_nonexistent_test__.png");
        let path_str = match nonexistent.to_str() {
            Some(s) => s,
            None => panic!("临时路径不是有效 UTF-8"),
        };
        match load_image(path_str, None) {
            Err(e) => assert!(e.to_string().contains("打开本地图片失败")),
            Ok(_) => panic!("不存在的文件应返回错误"),
        }
    }

    /// 测试 4：相对路径但未提供 working_dir 返回错误。
    #[test]
    fn load_from_local_relative_without_working_dir() {
        match load_image("relative/image.png", None) {
            Err(e) => assert!(e.to_string().contains("相对路径图片需要设置 working_dir")),
            Ok(_) => panic!("无 working_dir 的相对路径应返回错误"),
        }
    }

    /// 测试 5：半透明 RGBA 图片展平后无 alpha 通道。
    #[test]
    fn alpha_image_is_flattened() {
        use image::{Rgba, RgbaImage};

        let mut rgba: RgbaImage = RgbaImage::new(2, 2);
        rgba.put_pixel(0, 0, Rgba([255, 0, 0, 128]));
        rgba.put_pixel(1, 0, Rgba([0, 255, 0, 128]));
        rgba.put_pixel(0, 1, Rgba([0, 0, 255, 128]));
        rgba.put_pixel(1, 1, Rgba([255, 255, 0, 128]));
        let dyn_img = DynamicImage::ImageRgba8(rgba);
        let flattened = flatten_alpha(dyn_img);
        // 展平后应为 RGB8，无 alpha 通道
        assert!(!flattened.color().has_alpha());
        assert_eq!(flattened.width(), 2);
        assert_eq!(flattened.height(), 2);
    }
}
