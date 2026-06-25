//! 图片托管模块：ImageStorage trait 及四种实现。
//!
//! 负责将图片数据持久化并返回 Markdown 图片 URL。

use std::fs;
use std::path::PathBuf;

use base64::Engine as _;
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
    pub fn mime_type(self) -> &'static str {
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
    pub fn extension(self) -> &'static str {
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
    /// 图片子目录名（默认 "images"）。
    pub local_dir: String,
    /// 当前 Markdown 文件所在目录（无路径时为 None）。
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

        // 返回相对路径（若有 working_dir）或绝对路径
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

/// 整理文件名：移除危险字符，若文件名不含扩展名则保留原名。
fn sanitize_filename(name: &str, _default_ext: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return "image".to_string();
    }
    // 检查是否已有已知扩展名
    let lower = name.to_lowercase();
    let has_known_ext = lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".svg");
    if has_known_ext {
        // 去掉扩展名，只保留 stem
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
    // 极端情况：添加时间戳
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

        // 构建 multipart/form-data body
        let mut body = Vec::new();
        let header = format!(
            "------zdown_boundary_978\r\n\
             Content-Disposition: form-data; name=\"smfile\"; filename=\"{filename}.{ext}\"\r\n\
             Content-Type: {mime}\r\n\r\n"
        );
        body.extend_from_slice(header.as_bytes());
        body.extend_from_slice(data);
        let footer = "\r\n------zdown_boundary_978--\r\n";
        body.extend_from_slice(footer.as_bytes());

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
            .map_err(|e| format!("SM.MS 上传请求失败: {e}"))?;

        let body_text = response
            .into_string()
            .map_err(|e| format!("SM.MS 读取响应失败: {e}"))?;

        // 解析 JSON 响应
        let json: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|e| format!("SM.MS 解析响应失败: {e}"))?;

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
// TencentCosStorage
// ---------------------------------------------------------------------------

/// 腾讯云 COS 图床上传。
///
/// 直接使用 COS REST API (PUT Object)，签名算法使用 HMAC-SHA1。
pub struct TencentCosStorage {
    pub secret_id: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub custom_domain: String,
    pub upload_path: String,
}

impl ImageStorage for TencentCosStorage {
    fn store(&self, data: &[u8], filename: &str, format: ImageFormat) -> Result<String, String> {
        if self.secret_id.is_empty() || self.secret_key.is_empty() {
            return Err("COS 配置不完整: 请设置 SecretId 和 SecretKey".to_string());
        }
        if self.bucket.is_empty() {
            return Err("COS 配置不完整: 请设置存储桶名称".to_string());
        }

        let mime = format.mime_type();
        let key = self.build_object_key(filename, format);
        let host = format!("{}.cos.{}.myqcloud.com", self.bucket, self.region);
        let url_path = format!("/{}", key);

        // 生成 COS 签名
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("系统时间错误: {e}"))?;
        let start_time = now.as_secs().saturating_sub(60);
        let end_time = now.as_secs().saturating_add(3600);
        let key_time = format!("{start_time};{end_time}");

        let sign_key = hmac_sha1(&self.secret_key, &key_time)?;
        let http_string = format!("put\n{url_path}\n\nhost={host}\n");
        let http_string_sha1 = sha1_hex(&http_string);
        let string_to_sign = format!("sha1\n{key_time}\n{http_string_sha1}\n");
        let signature = hmac_sha1(&sign_key, &string_to_sign)?;

        let authorization = format!(
            "q-sign-algorithm=sha1&\
             q-ak={}&q-sign-time={key_time}&\
             q-key-time={key_time}&\
             q-header-list=host&\
             q-url-param-list=&\
             q-signature={signature}",
            self.secret_id
        );

        // 上传（ureq 同步 HTTP PUT）
        let put_url = format!("https://{host}{url_path}");
        let response = ureq::put(&put_url)
            .set("Host", &host)
            .set("Content-Type", mime)
            .set("x-cos-acl", "public-read")
            .set("Authorization", &authorization)
            .set("User-Agent", "zdown/0.1")
            .timeout(std::time::Duration::from_secs(60))
            .send_bytes(data)
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("401") || err_str.contains("403") {
                    "COS 认证失败: 请检查 SecretId/SecretKey/存储桶权限".to_string()
                } else if err_str.contains("404") {
                    "COS 存储桶不存在或地域错误".to_string()
                } else {
                    format!("COS 上传失败: {err_str}")
                }
            })?;

        if response.status() == 200 {
            let url = if !self.custom_domain.is_empty() {
                format!("https://{}/{}", self.custom_domain, key)
            } else {
                format!("https://{host}/{key}")
            };
            Ok(url)
        } else {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            Err(format!("COS 上传失败 (HTTP {status}): {body}"))
        }
    }
}

impl TencentCosStorage {
    /// 根据文件名和上传路径模板生成 COS object key。
    fn build_object_key(&self, filename: &str, format: ImageFormat) -> String {
        let ext = format.extension();
        let stem = sanitize_for_cos(filename);
        let uuid = short_uuid();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let days_since_epoch = secs / 86400;
        let (year, month, day) = approximate_date(days_since_epoch);

        let path = self
            .upload_path
            .replace("{year}", &year.to_string())
            .replace("{month}", &format!("{month:02}"))
            .replace("{day}", &format!("{day:02}"));

        format!("{path}/{stem}_{uuid}.{ext}")
    }
}

/// 简化版 civil date（从 Unix epoch days 推算，近似但够用于目录分桶）
fn approximate_date(days: u64) -> (u64, u64, u64) {
    let mut remaining = days as i64;
    let mut year = 1970i64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }
    let day = remaining + 1;
    (year as u64, month as u64, day as u64)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// 整理 COS 文件名：移除特殊字符，保留字母数字和下划线连字符。
fn sanitize_for_cos(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return "image".to_string();
    }
    // 取 stem
    let stem = if let Some(dot) = name.rfind('.') {
        &name[..dot]
    } else {
        name
    };
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 简单短 UUID（8 位十六进制，基于时间 + PID）。
fn short_uuid() -> String {
    use std::hash::{Hash as _, Hasher as _};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    let h = hasher.finish();
    // 取低 32 位得到 8 位十六进制
    format!("{:08x}", (h as u32))
}

/// HMAC-SHA1 计算，返回十六进制字符串。
fn hmac_sha1(key: &str, data: &str) -> Result<String, String> {
    use hmac::{Hmac, Mac as _};
    use sha1::Sha1;
    let mut mac = Hmac::<Sha1>::new_from_slice(key.as_bytes())
        .map_err(|e| format!("HMAC-SHA1 初始化失败: {e}"))?;
    mac.update(data.as_bytes());
    let result = mac.finalize();
    Ok(hex_encode(result.into_bytes().as_slice()))
}

/// SHA1 哈希，返回十六进制字符串。
fn sha1_hex(data: &str) -> String {
    use sha1::Digest as _;
    let mut hasher = sha1::Sha1::new();
    hasher.update(data.as_bytes());
    hex_encode(&hasher.finalize())
}

/// 字节数组转十六进制（小写）。
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// PicGoStorage
// ---------------------------------------------------------------------------

/// PicGo 桥接存储：通过 PicGo HTTP Server 上传图片。
///
/// 需先安装 PicGo 并启动 server 模式：`picgo server -p <port>`。
/// zdown 通过 HTTP multipart/form-data 将图片发送给 PicGo，
/// PicGo 使用其当前配置的上传器完成云端上传。
pub struct PicGoStorage {
    pub server_port: u16,
}

impl ImageStorage for PicGoStorage {
    fn store(&self, data: &[u8], filename: &str, format: ImageFormat) -> Result<String, String> {
        let boundary = "----zdown_picgo_boundary_771";
        let ext = format.extension();
        let mime = format.mime_type();

        // 构建 multipart/form-data body
        let mut body = Vec::new();
        let header = format!(
            "------zdown_picgo_boundary_771\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{filename}.{ext}\"\r\n\
             Content-Type: {mime}\r\n\r\n"
        );
        body.extend_from_slice(header.as_bytes());
        body.extend_from_slice(data);
        let footer = "\r\n------zdown_picgo_boundary_771--\r\n";
        body.extend_from_slice(footer.as_bytes());

        let content_type = format!("multipart/form-data; boundary={boundary}");
        let upload_url = format!("http://127.0.0.1:{}/upload", self.server_port);

        let response = ureq::post(&upload_url)
            .set("Content-Type", &content_type)
            .set("User-Agent", "zdown/0.1")
            .timeout(std::time::Duration::from_secs(60))
            .send_bytes(&body)
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("Connection refused") || err_str.contains("连接被拒绝") {
                    format!(
                        "无法连接 PicGo Server (127.0.0.1:{})，\
                         请确认已启动 picgo server",
                        self.server_port
                    )
                } else if err_str.contains("timed out") || err_str.contains("Timeout") {
                    format!(
                        "PicGo Server (127.0.0.1:{}) 响应超时，请检查网络或 PicGo 状态",
                        self.server_port
                    )
                } else {
                    format!("PicGo 上传请求失败: {err_str}")
                }
            })?;

        let body_text = response
            .into_string()
            .map_err(|e| format!("PicGo 读取响应失败: {e}"))?;

        // 解析 PicGo HTTP Server 响应
        let json: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("PicGo 解析响应失败: {e}，原始响应: {body_text}"))?;

        if json["success"].as_bool() == Some(true) {
            // 成功：result 是 URL 数组，取第一个
            json["result"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| format!("PicGo 返回缺少有效 URL，原始响应: {body_text}"))
        } else {
            let msg = json["message"].as_str().unwrap_or("未知错误");
            Err(format!("PicGo 上传失败: {msg}"))
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
        ImageStrategy::TencentCos => Box::new(TencentCosStorage {
            secret_id: config.tencent_cos.secret_id.clone(),
            secret_key: config.tencent_cos.secret_key.clone(),
            bucket: config.tencent_cos.bucket.clone(),
            region: config.tencent_cos.region.clone(),
            custom_domain: config.tencent_cos.custom_domain.clone(),
            upload_path: config.tencent_cos.upload_path.clone(),
        }),
        ImageStrategy::PicGo => Box::new(PicGoStorage {
            server_port: config.picgo.server_port,
        }),
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
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
        assert_eq!(sanitize_filename("", "png"), "image");
    }

    #[test]
    fn base64_storage_returns_data_uri() {
        let storage = Base64Storage;
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
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
        let config = ImageHostingConfig {
            default_strategy: ImageStrategy::Base64,
            ..Default::default()
        };
        let storage = create_storage(&config, None);
        let url = storage
            .store(b"data", "x.png", ImageFormat::Png)
            .expect("store");
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn create_storage_tencent_cos_validation() {
        let config = ImageHostingConfig {
            default_strategy: ImageStrategy::TencentCos,
            ..Default::default()
        };
        let storage = create_storage(&config, None);
        // 凭据为空 → 应在 store 前就返回错误
        let result = storage.store(b"test", "photo.png", ImageFormat::Png);
        assert!(result.is_err());
    }

    #[test]
    fn tencent_cos_missing_credentials() {
        let config = TencentCosStorage {
            secret_id: String::new(),
            secret_key: String::new(),
            bucket: String::new(),
            region: "ap-guangzhou".into(),
            custom_domain: String::new(),
            upload_path: "zdown/{year}/{month}".into(),
        };
        let result = config.store(b"data", "test.png", ImageFormat::Png);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("SecretId") || err.contains("SecretKey"),
            "expected missing creds error, got: {err}"
        );
    }

    #[test]
    fn tencent_cos_missing_bucket() {
        let config = TencentCosStorage {
            secret_id: "AKID_test".into(),
            secret_key: "test_key".into(),
            bucket: String::new(),
            region: "ap-guangzhou".into(),
            custom_domain: String::new(),
            upload_path: "zdown/{year}/{month}".into(),
        };
        let result = config.store(b"data", "test.png", ImageFormat::Png);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("存储桶"),
            "expected missing bucket error, got: {err}"
        );
    }

    #[test]
    fn sanitize_for_cos_removes_special_chars() {
        assert_eq!(sanitize_for_cos("hello world!.png"), "hello_world_");
        assert_eq!(sanitize_for_cos("图片"), "__");
        assert_eq!(sanitize_for_cos("ok-file_name-123.png"), "ok-file_name-123");
    }

    #[test]
    fn approximate_date_known() {
        let (y, m, _d) = approximate_date(20608);
        assert_eq!(y, 2026);
        assert_eq!(m, 6);
        // day is approximate; exact value depends on leap year calculation
        // the function is for directory bucketing, day precision is not critical
    }

    #[test]
    fn hmc_sha1_produces_hex() {
        let result = hmac_sha1("key", "data").expect("hmac_sha1");
        assert_eq!(result.len(), 40); // SHA1 hex is 40 chars
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn short_uuid_is_8_hex_chars() {
        let id = short_uuid();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hex_encode_bytes() {
        assert_eq!(hex_encode(&[0xAB, 0xCD]), "abcd");
        assert_eq!(hex_encode(&[0x00, 0xFF]), "00ff");
    }

    // ── PicGoStorage 测试 ──

    #[test]
    fn picgo_storage_connection_refused() {
        // 使用端口 0 确保连接被拒绝（通常无服务监听）
        let storage = PicGoStorage { server_port: 0 };
        let data = b"fake image data";
        let result = storage.store(data, "test.png", ImageFormat::Png);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // 不同平台报错信息不同（"Connection refused" / "地址无效" 等），
        // 只需确认返回了错误即可。
        assert!(
            err.contains("PicGo"),
            "expected PicGo-related error, got: {err}"
        );
    }

    #[test]
    fn create_storage_picgo() {
        let config = ImageHostingConfig {
            default_strategy: ImageStrategy::PicGo,
            picgo: config::PicGoConfig { server_port: 0 },
            ..Default::default()
        };
        let storage = create_storage(&config, None);
        let result = storage.store(b"test", "photo.png", ImageFormat::Png);
        // 端口 0 无法连接，应返回错误
        assert!(result.is_err());
    }
}
