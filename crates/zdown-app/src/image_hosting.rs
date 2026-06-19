//! 图片托管模块：ImageStorage trait 及三种实现。
//!
//! 负责将图片数据持久化并返回 Markdown 图片 URL。

use std::fs;
use std::path::PathBuf;

use base64::Engine;
use config::{ImageHostingConfig, ImageStrategy};

// ---------------------------------------------------------------------------
// ImageFormat
// ---------------------------------------------------------------------------

/// 支持的图片格式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    WebP,
    Svg,
    Unknown,
}

impl ImageFormat {
    /// 从文件名推断图片格式。
    pub fn from_filename(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.ends_with(".png") {
            ImageFormat::Png
        } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            ImageFormat::Jpeg
        } else if lower.ends_with(".gif") {
            ImageFormat::Gif
        } else if lower.ends_with(".webp") {
            ImageFormat::WebP
        } else if lower.ends_with(".svg") {
            ImageFormat::Svg
        } else {
            ImageFormat::Unknown
        }
    }

    /// MIME type 字符串。
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Unknown => "application/octet-stream",
        }
    }

    /// 文件扩展名（不含点）。
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Gif => "gif",
            ImageFormat::WebP => "webp",
            ImageFormat::Svg => "svg",
            ImageFormat::Unknown => "bin",
        }
    }
}

// ---------------------------------------------------------------------------
// ImageStorage trait
// ---------------------------------------------------------------------------

/// 图片存储后端。
pub trait ImageStorage {
    /// 存储图片数据，返回 Markdown 中使用的 URL。
    fn store(&self, data: &[u8], filename: &str, format: ImageFormat) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// LocalStorage
// ---------------------------------------------------------------------------

/// 本地文件存储：复制到文档目录的 `images/` 子目录。
pub struct LocalStorage {
    pub local_dir: String,
    pub working_dir: Option<PathBuf>,
}

impl ImageStorage for LocalStorage {
    fn store(&self, data: &[u8], filename: &str, format: ImageFormat) -> Result<String, String> {
        let base_dir = match &self.working_dir {
            Some(dir) => dir.join(&self.local_dir),
            None => std::env::temp_dir().join("zdown_images"),
        };
        fs::create_dir_all(&base_dir).map_err(|e| format!("创建图片目录失败: {e}"))?;

        let ext = format.extension();
        let name = sanitize_filename(filename, ext);
        let dest = unique_path(&base_dir, &name, ext);

        fs::write(&dest, data).map_err(|e| format!("图片写入失败: {e}"))?;

        if self.working_dir.is_some() {
            let file_name = dest
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("{name}.{ext}"));
            Ok(format!("{}/{file_name}", self.local_dir))
        } else {
            Ok(dest.to_string_lossy().into_owned())
        }
    }
}

/// 整理文件名：移除特殊字符，若文件名不含扩展名则去掉扩展名部分。
fn sanitize_filename(name: &str, default_ext: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return format!("image.{default_ext}");
    }
    let lower = name.to_lowercase();
    let has_known_ext = lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".svg");
    if has_known_ext {
        let stem = &name[..name.rfind('.').unwrap_or(name.len())];
        stem.to_string()
    } else {
        name.to_string()
    }
}

/// 确保文件路径唯一：若存在则添加 `_1`, `_2` 后缀。
fn unique_path(dir: &std::path::Path, name: &str, ext: &str) -> PathBuf {
    let candidate = dir.join(format!("{name}.{ext}"));
    if !candidate.exists() {
        return candidate;
    }
    for i in 1..1000 {
        let alt = dir.join(format!("{name}_{i}.{ext}"));
        if !alt.exists() {
            return alt;
        }
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dir.join(format!("{name}_{ts}.{ext}"))
}

// ---------------------------------------------------------------------------
// Base64Storage
// ---------------------------------------------------------------------------

/// Base64 内联存储：直接编码为 data URI。
pub struct Base64Storage;

impl ImageStorage for Base64Storage {
    fn store(&self, data: &[u8], _filename: &str, format: ImageFormat) -> Result<String, String> {
        let mime = format.mime_type();
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        Ok(format!("data:{mime};base64,{encoded}"))
    }
}

// ---------------------------------------------------------------------------
// SmMsStorage
// ---------------------------------------------------------------------------

/// SM.MS 云端图床上传。
pub struct SmMsStorage {
    pub api_token: String,
}

impl ImageStorage for SmMsStorage {
    fn store(&self, data: &[u8], filename: &str, format: ImageFormat) -> Result<String, String> {
        let boundary = "----zdown_boundary_978";
        let ext = format.extension();
        let mime = format.mime_type();

        let mut body = Vec::new();
        let header = format!(
            "------zdown_boundary_978\r\nContent-Disposition: form-data; name=\"smfile\"; filename=\"{filename}.{ext}\"\r\nContent-Type: {mime}\r\n\r\n"
        );
        body.extend_from_slice(header.as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice("\r\n------zdown_boundary_978--\r\n".as_bytes());

        let content_type = format!("multipart/form-data; boundary={boundary}");

        let mut request = ureq::post("https://sm.ms/api/v2/upload")
            .set("Content-Type", &content_type)
            .set("User-Agent", "zdown/0.1");
        if !self.api_token.is_empty() {
            request = request.set("Authorization", &self.api_token);
        }
        let response = request
            .timeout(std::time::Duration::from_secs(30))
            .send_bytes(&body)
            .map_err(|e| format!("上传请求失败: {e}"))?;

        let body_text = response
            .into_string()
            .map_err(|e| format!("读取响应失败: {e}"))?;

        let json: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|e| format!("解析响应失败: {e}"))?;

        if json["success"].as_bool() == Some(true) {
            json["data"]["url"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "SM.MS 返回缺少 URL".to_string())
        } else if json["code"].as_str() == Some("image_repeated") {
            json["images"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "SM.MS 重复图片返回缺少 URL".to_string())
        } else {
            let msg = json["message"].as_str().unwrap_or("未知错误");
            Err(format!("SM.MS 上传失败: {msg}"))
        }
    }
}

// ---------------------------------------------------------------------------
// 工厂函数
// ---------------------------------------------------------------------------

/// 根据配置创建对应的 ImageStorage 实现。
pub fn create_storage(
    config: &ImageHostingConfig,
    working_dir: Option<PathBuf>,
) -> Box<dyn ImageStorage> {
    match config.default_strategy {
        ImageStrategy::Local => Box::new(LocalStorage {
            local_dir: config.local_dir.clone(),
            working_dir,
        }),
        ImageStrategy::Base64 => Box::new(Base64Storage),
        ImageStrategy::SmMs => Box::new(SmMsStorage {
            api_token: config.smms.api_token.clone(),
        }),
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_format_from_filename_png() {
        assert_eq!(ImageFormat::from_filename("photo.png"), ImageFormat::Png);
    }

    #[test]
    fn image_format_from_filename_jpg() {
        assert_eq!(ImageFormat::from_filename("photo.jpg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_filename("photo.jpeg"), ImageFormat::Jpeg);
    }

    #[test]
    fn image_format_from_filename_unknown() {
        assert_eq!(ImageFormat::from_filename("file.xyz"), ImageFormat::Unknown);
    }

    #[test]
    fn image_format_mime_type() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
    }

    #[test]
    fn image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn sanitize_filename_with_extension() {
        assert_eq!(sanitize_filename("photo.png", "png"), "photo");
    }

    #[test]
    fn sanitize_filename_without_extension() {
        assert_eq!(sanitize_filename("photo", "png"), "photo");
    }

    #[test]
    fn sanitize_filename_empty() {
        assert_eq!(sanitize_filename("", "png"), "image.png");
    }

    #[test]
    fn base64_storage_returns_data_uri() {
        let storage = Base64Storage;
        let data = vec![0x89, 0x50, 0x4E, 0x47];
        let url = storage
            .store(&data, "test.png", ImageFormat::Png)
            .expect("store");
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn local_storage_creates_file() {
        let tmp = std::env::temp_dir().join("zdown_test_local_storage");
        let _ = fs::remove_dir_all(&tmp);
        let storage = LocalStorage {
            local_dir: "images".into(),
            working_dir: Some(tmp.clone()),
        };
        let data = b"fake png data";
        let url = storage
            .store(data, "icon.png", ImageFormat::Png)
            .expect("store");
        assert!(url.starts_with("images/"));
        assert!(url.ends_with(".png"));
        let file_path = tmp.join(&url);
        assert!(file_path.exists(), "file should exist at {file_path:?}");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn local_storage_dedup_filename() {
        let tmp = std::env::temp_dir().join("zdown_test_dedup");
        let _ = fs::remove_dir_all(&tmp);
        let storage = LocalStorage {
            local_dir: "img".into(),
            working_dir: Some(tmp.clone()),
        };
        let data = b"data1";
        let url1 = storage
            .store(data, "pic.png", ImageFormat::Png)
            .expect("store1");
        let url2 = storage
            .store(data, "pic.png", ImageFormat::Png)
            .expect("store2");
        assert_ne!(url1, url2, "dedup should create different filenames");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn create_storage_local() {
        let config = ImageHostingConfig::default();
        let storage = create_storage(&config, None);
        let result = storage.store(b"test", "x.png", ImageFormat::Png);
        assert!(result.is_ok());
    }

    #[test]
    fn create_storage_base64() {
        let mut config = ImageHostingConfig::default();
        config.default_strategy = ImageStrategy::Base64;
        let storage = create_storage(&config, None);
        let url = storage
            .store(b"data", "x.png", ImageFormat::Png)
            .expect("store");
        assert!(url.starts_with("data:image/png;base64,"));
    }
}
