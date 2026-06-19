# 自定义图床 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 实现图片插入（拖拽/粘贴/浏览）和三种存储后端（本地复制/base64内联/SM.MS云端）

**架构：** 新增 `zdown-app/src/image_hosting.rs`（`ImageStorage` trait + 3 种实现），扩展 `AppConfig` 增加 `ImageHostingConfig`，修改 `source_view.rs`/`input.rs` 处理拖拽粘贴事件，修改 `settings_dialog.rs` 增加"图片"标签页

**技术栈：** Rust 2024, egui 0.34, ureq 2, base64 0.22, arboard 3

---

### 任务 1：添加依赖

**文件：**
- 修改：`crates/zdown-app/Cargo.toml`

- [ ] **步骤 1：添加 ureq、base64、arboard 依赖**

```toml
[dependencies]
eframe.workspace = true
egui.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
editor_engine.workspace = true
workspace.workspace = true
document_model.workspace = true
markdown_renderer.workspace = true
export_engine.workspace = true
config.workspace = true
ureq = { version = "2", default-features = false, features = ["tls"] }
base64 = "0.22"
arboard = "3"
image = "0.23"
open = "5"
```

- [ ] **步骤 2：验证编译**

```bash
cargo check -p zdown-app
```

预期：编译成功（新增 crate 下载并解析）

- [ ] **步骤 3：Commit**

```bash
git add crates/zdown-app/Cargo.toml && git commit -m "build(zdown-app): add ureq, base64, arboard deps for image hosting"
```

---

### 任务 2：扩展 AppConfig

**文件：**
- 修改：`crates/config/src/lib.rs`

- [ ] **步骤 1：编写 ImageHostingConfig 测试（TDD 红灯）**

在 `crates/config/src/lib.rs` 的 `#[cfg(test)] mod tests` 块末尾添加：

```rust
#[test]
fn image_hosting_config_default() {
    let config = ImageHostingConfig::default();
    assert!(matches!(config.default_strategy, ImageStrategy::Local));
    assert_eq!(config.local_dir, "images");
    assert_eq!(config.smms.api_token, "");
}

#[test]
fn image_hosting_config_roundtrip() {
    let path = temp_path("image_hosting");
    cleanup(&path);

    let config = AppConfig {
        custom_css: None,
        theme: ThemeMode::Dark,
        image_hosting: ImageHostingConfig {
            default_strategy: ImageStrategy::Base64,
            local_dir: "assets".into(),
            smms: SmMsConfig {
                api_token: "token123".into(),
            },
        },
    };
    config.save_to(&path).expect("save");
    let loaded = AppConfig::load_from(&path).expect("load");
    assert!(matches!(loaded.image_hosting.default_strategy, ImageStrategy::Base64));
    assert_eq!(loaded.image_hosting.local_dir, "assets");
    assert_eq!(loaded.image_hosting.smms.api_token, "token123");
    cleanup(&path);
}

#[test]
fn old_config_without_image_hosting_defaults() {
    let path = temp_path("old_img_config");
    cleanup(&path);
    std::fs::write(&path, "custom_css = \"h1 { color: red; }\"\n").expect("write");
    let loaded = AppConfig::load_from(&path).expect("load");
    assert!(matches!(loaded.image_hosting.default_strategy, ImageStrategy::Local));
    assert_eq!(loaded.image_hosting.local_dir, "images");
    cleanup(&path);
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p config -- image_hosting
```

预期：编译错误，`ImageHostingConfig`/`ImageStrategy`/`SmMsConfig` 未定义

- [ ] **步骤 3：实现 ImageStrategy、SmMsConfig、ImageHostingConfig**

在 `AppConfig` 结构体之前添加：

```rust
/// 图片存储策略。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum ImageStrategy {
    #[default]
    Local,
    Base64,
    SmMs,
}

/// SM.MS 图床配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SmMsConfig {
    /// SM.MS API token（可选，无 token 也可上传但有限制）。
    pub api_token: String,
}

/// 图片托管配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageHostingConfig {
    /// 默认存储策略。
    pub default_strategy: ImageStrategy,
    /// 本地图片子目录名。
    pub local_dir: String,
    /// SM.MS 配置。
    pub smms: SmMsConfig,
}

impl Default for ImageHostingConfig {
    fn default() -> Self {
        Self {
            default_strategy: ImageStrategy::Local,
            local_dir: "images".into(),
            smms: SmMsConfig::default(),
        }
    }
}
```

在 `AppConfig` 增加字段（`theme` 之后）：

```rust
pub struct AppConfig {
    pub custom_css: Option<String>,
    pub theme: ThemeMode,
    /// 图片托管配置。
    pub image_hosting: ImageHostingConfig,
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p config -- image_hosting
```

预期：3 个新测试 PASS

- [ ] **步骤 5：确保现有测试未破坏**

```bash
cargo test -p config
```

预期：全部 15 个测试 PASS

- [ ] **步骤 6：Commit**

```bash
git add crates/config/src/lib.rs && git commit -m "feat(config): add ImageHostingConfig with Local/Base64/SmMs strategies"
```

---

### 任务 3：创建 image_hosting 模块

**文件：**
- 创建：`crates/zdown-app/src/image_hosting.rs`
- 修改：`crates/zdown-app/src/lib.rs`（或 `main.rs`，取决于模块声明方式）

- [ ] **步骤 1：编写测试**

在将要创建的 `crates/zdown-app/src/image_hosting.rs` 中先写测试：

```rust
//! 图片托管模块：ImageStorage trait 及三种实现。
//!
//! 负责将图片数据持久化并返回 Markdown 图片 URL。

use std::fs;
use std::path::PathBuf;

use base64::Engine;

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
        fs::create_dir_all(&base_dir)
            .map_err(|e| format!("创建图片目录失败: {e}"))?;

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

/// 整理文件名：移除特殊字符，若文件名不含扩展名则补上。
fn sanitize_filename(name: &str, default_ext: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        return format!("image.{default_ext}");
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
        let stem = &name[..name.rfind('.').unwrap()];
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
            "------zdown_boundary_978\r\nContent-Disposition: form-data; name=\"smfile\"; filename=\"{filename}.{ext}\"\r\nContent-Type: {mime}\r\n\r\n"
        );
        body.extend_from_slice(header.as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n------zdown_boundary_978--\r\n").as_bytes());

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

        // 解析 JSON 响应
        let json: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("解析响应失败: {e}"))?;

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
            let msg = json["message"]
                .as_str()
                .unwrap_or("未知错误");
            Err(format!("SM.MS 上传失败: {msg}"))
        }
    }
}

// ---------------------------------------------------------------------------
// 工厂函数
// ---------------------------------------------------------------------------

use config::{ImageHostingConfig, ImageStrategy};

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
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
        let url = storage.store(&data, "test.png", ImageFormat::Png).expect("store");
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
        let url = storage.store(data, "icon.png", ImageFormat::Png).expect("store");
        assert!(url.starts_with("images/"));
        assert!(url.ends_with(".png"));
        // 验证文件存在
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
        let url1 = storage.store(data, "pic.png", ImageFormat::Png).expect("store1");
        let url2 = storage.store(data, "pic.png", ImageFormat::Png).expect("store2");
        assert_ne!(url1, url2, "dedup should create different filenames");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn create_storage_local() {
        let config = ImageHostingConfig::default();
        let storage = create_storage(&config, None);
        // 通过 store 行为验证类型正确（会尝试创建目录或失败）
        let result = storage.store(b"test", "x.png", ImageFormat::Png);
        // 无 working_dir 时写入临时目录，应成功
        assert!(result.is_ok());
    }

    #[test]
    fn create_storage_base64() {
        let mut config = ImageHostingConfig::default();
        config.default_strategy = ImageStrategy::Base64;
        let storage = create_storage(&config, None);
        let url = storage.store(b"data", "x.png", ImageFormat::Png).expect("store");
        assert!(url.starts_with("data:image/png;base64,"));
    }
}
```

- [ ] **步骤 2：在 main.rs 中声明模块**

修改 `crates/zdown-app/src/main.rs`，在现有 `mod` 声明处添加：

```rust
mod image_hosting;
```

- [ ] **步骤 3：运行测试验证通过**

```bash
cargo test -p zdown-app -- image_hosting
```

预期：13 个测试 PASS

- [ ] **步骤 4：确保编译通过**

```bash
cargo check -p zdown-app
```

预期：编译成功

- [ ] **步骤 5：Commit**

```bash
git add crates/zdown-app/src/image_hosting.rs crates/zdown-app/src/main.rs && git commit -m "feat(zdown-app): add ImageStorage trait with Local/Base64/SmMs backends"
```

---

### 任务 4：浏览插入图片（Ctrl+I）

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`

- [ ] **步骤 1：实现 trigger_browse_image 函数**

在 `trigger_export_html` 函数之后添加：

```rust
/// 浏览选择图片文件，按默认策略插入到编辑器。
fn trigger_browse_image(state: &mut EditorState, config: &ImageHostingConfig) {
    let path = match workspace::pick_open_image() {
        Some(p) => p,
        None => return,
    };

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image.png".to_string());

    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            state.status_message = format!("图片读取失败: {e}");
            return;
        }
    };

    let format = crate::image_hosting::ImageFormat::from_filename(&filename);
    let working_dir = state.current_path().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let storage = crate::image_hosting::create_storage(config, working_dir);

    match storage.store(&data, &filename, format) {
        Ok(url) => {
            let md_text = format!("![{filename}]({url})");
            let cursor = state.editor().cursor;
            if state
                .apply(editor_engine::Command::Insert {
                    pos: cursor,
                    text: md_text,
                })
                .is_err()
            {
                state.status_message = "图片插入失败".to_string();
            }
        }
        Err(e) => {
            state.status_message = format!("图片存储失败: {e}");
        }
    }
}
```

- [ ] **步骤 2：在 workspace 添加 pick_open_image 对话框**

修改 `crates/workspace/src/dialog.rs`，在 `pick_save_file_html` 之后添加：

```rust
/// 弹出打开图片文件对话框。用户取消或环境不支持时返回 `None`。
pub fn pick_open_image() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("图片", &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"])
        .set_title("选择图片")
        .pick_file()
}
```

在 `crates/workspace/src/lib.rs` 的 `pub use` 行添加 `pick_open_image`：

```rust
pub use dialog::{pick_open_file, pick_open_image, pick_save_file, pick_save_file_html, pick_save_file_pdf};
```

- [ ] **步骤 3：在菜单中添加"插入图片"按钮**

修改 `crates/zdown-app/src/menu.rs` 的 `show_menu` 函数，在编辑菜单的撤销/重做之后添加：

找到以下位置（约 line 120）：
```rust
            ui.menu_button("编辑", |ui| {
                if ui.button("撤销 (Ctrl+Z)").clicked() {
                    let _ = state.undo();
                }
                if ui.button("重做 (Ctrl+Y)").clicked() {
                    let _ = state.redo();
                }
            });
```

在重做按钮之后、`});` 之前增加：
```rust
                ui.separator();
                if ui.button("插入图片... (Ctrl+I)").clicked() {
                    trigger_browse_image(state, &app_config.image_hosting);
                    ui.close();
                }
```

- [ ] **步骤 4：需要将 app_config 改为传递给 show_menu**

当前 `show_menu` 签名中 `app_config: &AppConfig` 已经是引用，无需修改。`trigger_browse_image` 接收 `&ImageHostingConfig`。

- [ ] **步骤 5：在 main.rs 添加 Ctrl+I 快捷键**

修改 `crates/zdown-app/src/main.rs`，在 Ctrl+F 快捷键处理之后添加：

```rust
        // Ctrl+I 浏览插入图片
        if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::I)) {
            menu::trigger_browse_image(&mut self.state, &self.app_config.image_hosting);
        }
```

需要将 `trigger_browse_image` 设为 `pub(crate)`：

修改 `menu.rs` 中函数签名：
```rust
pub(crate) fn trigger_browse_image(state: &mut EditorState, config: &ImageHostingConfig) {
```

并在 `menu.rs` 顶部添加 import：
```rust
use config::ImageHostingConfig;
```

- [ ] **步骤 6：编译验证**

```bash
cargo check -p zdown-app
```

预期：编译成功

- [ ] **步骤 7：Commit**

```bash
git add crates/zdown-app/src/menu.rs crates/zdown-app/src/main.rs crates/workspace/src/dialog.rs crates/workspace/src/lib.rs && git commit -m "feat(zdown-app): add Ctrl+I browse image insertion with pick_open_image dialog"
```

---

### 任务 5：拖拽插入图片

**文件：**
- 修改：`crates/zdown-app/src/input.rs`
- 修改：`crates/zdown-app/src/source_view.rs`

- [ ] **步骤 1：在 input.rs 添加 handle_dropped_files 函数**

在 `crates/zdown-app/src/input.rs` 末尾添加：

```rust
/// 处理拖拽的图片文件，插入到编辑器。
/// 返回实际插入的图片数量。
pub(crate) fn handle_dropped_images(
    ctx: &egui::Context,
    editor: &mut Editor,
    config: &config::ImageHostingConfig,
    working_dir: Option<std::path::PathBuf>,
) -> usize {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    if dropped.is_empty() {
        return 0;
    }

    let storage = crate::image_hosting::create_storage(config, working_dir);
    let mut inserted = 0;

    for file in &dropped {
        // 过滤非图片
        let mime = file.mime.to_lowercase();
        if !mime.starts_with("image/") {
            continue;
        }
        let data = match &file.bytes {
            Some(b) => b.clone(),
            None => {
                // 尝试从路径读取
                match &file.path {
                    Some(p) => match std::fs::read(p) {
                        Ok(b) => b,
                        Err(_) => continue,
                    },
                    None => continue,
                }
            }
        };
        let name = file
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.name.clone());

        let format = crate::image_hosting::ImageFormat::from_filename(&name);

        match storage.store(&data, &name, format) {
            Ok(url) => {
                let md_text = if inserted == 0 {
                    format!("![{name}]({url})")
                } else {
                    format!("\n![{name}]({url})")
                };
                let cursor = editor.cursor;
                let _ = editor.apply(Command::Insert { pos: cursor, text: md_text });
                inserted += 1;
            }
            Err(_) => {
                // 跳过失败的图片，继续处理下一个
            }
        }
    }

    inserted
}
```

并在文件顶部添加 import：

```rust
use config;
```

- [ ] **步骤 2：在 source_view 中调用拖拽处理**

修改 `crates/zdown-app/src/source_view.rs` 的 `show_source_view` 函数签名，新增 `app_config` 参数：

```rust
pub fn show_source_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    app_config: &config::ImageHostingConfig,
) {
```

在函数体开头（`let src = ...` 之前）添加拖拽处理：

```rust
    let working_dir = state.current_path().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    crate::input::handle_dropped_images(ui.ctx(), state.editor_mut(), app_config, working_dir);
```

- [ ] **步骤 3：同样修改 hybrid_view 和 preview_view 的签名**

修改 `crates/zdown-app/src/hybrid_view.rs` 的 `show_hybrid_view` 签名：

```rust
pub fn show_hybrid_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    app_config: &config::ImageHostingConfig,
) {
```

在函数体开头添加相同的拖拽处理代码。

修改 `crates/zdown-app/src/preview_view.rs` 的 `show_preview_view` 签名：

```rust
pub fn show_preview_view(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    app_config: &config::ImageHostingConfig,
) {
```

在函数体开头添加相同的拖拽处理代码。

- [ ] **步骤 4：更新 main.rs 中的调用**

修改 `crates/zdown-app/src/main.rs`，更新三处视图函数调用，传入 `&self.app_config.image_hosting`。

找到 `show_source_view(ui, &mut self.state, highlighter, &self.search)` 行，改为：
```rust
show_source_view(ui, &mut self.state, highlighter, &self.search, &self.app_config.image_hosting);
```

类似修改 `show_preview_view` 和 `show_hybrid_view` 的调用。

- [ ] **步骤 5：编译验证**

```bash
cargo check -p zdown-app
```

预期：编译成功

- [ ] **步骤 6：Commit**

```bash
git add crates/zdown-app/src/input.rs crates/zdown-app/src/source_view.rs crates/zdown-app/src/hybrid_view.rs crates/zdown-app/src/preview_view.rs crates/zdown-app/src/main.rs && git commit -m "feat(zdown-app): add drag-and-drop image insertion in all view modes"
```

---

### 任务 6：粘贴图片插入

**文件：**
- 修改：`crates/zdown-app/src/input.rs`

- [ ] **步骤 1：添加 handle_clipboard_image 函数**

在 `crates/zdown-app/src/input.rs` 的 `handle_dropped_images` 之后添加：

```rust
/// 尝试从剪贴板读取图片并插入到编辑器。
/// 返回 `true` 如果插入了图片（此时不应再处理文本粘贴）。
pub(crate) fn try_paste_image(
    editor: &mut Editor,
    config: &config::ImageHostingConfig,
    working_dir: Option<std::path::PathBuf>,
) -> bool {
    let clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let image_data = match clipboard.get_image() {
        Ok(img) => img,
        Err(_) => return false,
    };

    // RGBA → PNG 编码
    let png_bytes = rgba_to_png(&image_data);
    let storage = crate::image_hosting::create_storage(config, working_dir);
    let filename = "clipboard_image";
    let format = crate::image_hosting::ImageFormat::Png;

    match storage.store(&png_bytes, filename, format) {
        Ok(url) => {
            let md_text = format!("![image]({url})");
            let cursor = editor.cursor;
            let _ = editor.apply(Command::Insert {
                pos: cursor,
                text: md_text,
            });
            true
        }
        Err(_) => false,
    }
}

/// 将 arboard ImageData (RGBA) 编码为 PNG 字节。
fn rgba_to_png(img: &arboard::ImageData) -> Vec<u8> {
    use std::io::Write;
    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_data, img.width as u32, img.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&img.bytes).unwrap();
    }
    png_data
}
```

**注意**：`rgba_to_png` 需要 `png` crate。在 `Cargo.toml` 添加：

```toml
png = "0.17"
```

或者使用 `image` crate（已在 export_engine 中依赖）来编码：

```rust
fn rgba_to_png(img: &arboard::ImageData) -> Vec<u8> {
    let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.clone())
        .expect("clipboard image data");
    let mut png_data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut png_data, image::ImageFormat::Png)
        .expect("png encode");
    png_data.into_inner()
}
```

使用 `image` crate 方案（已在依赖树中），需要在 `Cargo.toml` 添加：

```toml
image = "0.23"
```

- [ ] **步骤 2：在 handle_input 中集成粘贴检测**

修改 `crates/zdown-app/src/input.rs` 的 `handle_input` 函数，不再处理文本粘贴逻辑——粘贴检测改为在调用侧处理。实际上更好的方案是：在 `handle_input` 中检测 `Event::Paste` 并跳过（由调用方单独处理粘贴）。

更简洁的方案：在 `handle_input` 中，当检测到 `Event::Paste(text)` 时，先尝试粘贴图片，仅当无图片时走文本粘贴逻辑。

修改 `handle_input` 签名和实现，添加参数：

```rust
pub(crate) fn handle_input(
    ctx: &egui::Context,
    editor: &mut Editor,
    config: &config::ImageHostingConfig,
    working_dir: Option<std::path::PathBuf>,
) {
```

修改 `Event::Text(text)` 分支和新增 `Event::Paste` 分支… 

实际更简洁的做法：由于 egui 0.34 中 `Event::Paste(String)` 和 `Event::Text(String)` 是分开的，`Text` 事件来自 IME 输入，`Paste` 事件来自 Ctrl+V。我们可以修改粘贴检测逻辑。

修改 `handle_input` 函数，在事件循环中，检测 `Event::Paste`：

```rust
            egui::Event::Paste(text) => {
                // 尝试剪贴板图片粘贴
                if crate::input::try_paste_image(editor, config, working_dir.clone()) {
                    // 图片已粘贴，跳过文本
                    continue;
                }
                // 无图片 → 文本粘贴
                if !text.is_empty() {
                    let cursor = editor.cursor;
                    let _ = editor.apply(Command::Insert { pos: cursor, text });
                }
            }
```

- [ ] **步骤 3：更新 handle_input 的所有调用方**

修改 `source_view.rs` 中 `handle_input` 调用，传入 config：

```rust
crate::input::handle_input(&ctx, state.editor_mut(), app_config, working_dir);
```

修改 `hybrid_view.rs` 中类似调用。

- [ ] **步骤 4：编译验证**

```bash
cargo check -p zdown-app
```

预期：编译成功

- [ ] **步骤 5：Commit**

```bash
git add crates/zdown-app/src/input.rs crates/zdown-app/src/source_view.rs crates/zdown-app/src/hybrid_view.rs crates/zdown-app/Cargo.toml && git commit -m "feat(zdown-app): add clipboard image paste with arboard"
```

---

### 任务 7：扩展设置对话框

**文件：**
- 修改：`crates/zdown-app/src/settings_dialog.rs`

- [ ] **步骤 1：重构 SettingsDialog 支持多标签页**

修改 `SettingsDialog` 结构体：

```rust
/// 设置对话框标签页。
#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsTab {
    Css,
    Image,
}

/// 设置对话框状态。
#[derive(Debug, Clone)]
pub struct SettingsDialog {
    pub open: bool,
    active_tab: SettingsTab,
    css_buffer: String,
    // 图片设置缓冲区
    local_dir_buffer: String,
    smms_token_buffer: String,
    strategy_buffer: usize, // 0=Local, 1=Base64, 2=SmMs
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: SettingsTab::Css,
            css_buffer: String::new(),
            local_dir_buffer: "images".to_string(),
            smms_token_buffer: String::new(),
            strategy_buffer: 0,
        }
    }
}

impl SettingsDialog {
    pub fn open_dialog(&mut self, current_css: Option<&str>, image_config: &ImageHostingConfig) {
        self.open = true;
        self.active_tab = SettingsTab::Css;
        self.css_buffer = current_css.unwrap_or("").to_string();
        self.local_dir_buffer = image_config.local_dir.clone();
        self.smms_token_buffer = image_config.smms.api_token.clone();
        self.strategy_buffer = match image_config.default_strategy {
            ImageStrategy::Local => 0,
            ImageStrategy::Base64 => 1,
            ImageStrategy::SmMs => 2,
        };
    }
}
```

- [ ] **步骤 2：重写 show_settings_dialog 支持标签页切换**

在 `show_settings_dialog` 函数中，添加标签栏和图片标签页：

在 `egui::Window::new("设置")` 内部，`resizable(true)` 之后，添加标签栏：

```rust
            ui.horizontal(|ui| {
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Css, "样式");
                ui.selectable_value(&mut dialog.active_tab, SettingsTab::Image, "图片");
            });
            ui.separator();

            match dialog.active_tab {
                SettingsTab::Css => {
                    // 原有的 CSS 编辑 UI（保持不变）
                    ui.label("自定义 CSS（追加到内置样式之后，留空表示不使用）：");
                    // ...
                }
                SettingsTab::Image => {
                    // 策略选择
                    ui.label("默认存储策略：");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut dialog.strategy_buffer, 0, "本地");
                        ui.selectable_value(&mut dialog.strategy_buffer, 1, "Base64");
                        ui.selectable_value(&mut dialog.strategy_buffer, 2, "SM.MS");
                    });
                    ui.add_space(8.0);

                    // 本地目录
                    ui.label("本地图片目录：");
                    ui.text_edit_singleline(&mut dialog.local_dir_buffer);
                    ui.add_space(8.0);

                    // SM.MS Token
                    ui.label("SM.MS API Token：");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut dialog.smms_token_buffer);
                        if ui.button("获取 Token").clicked() {
                            // 打开浏览器
                            let _ = open::that("https://sm.ms/home/apitoken");
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("无 Token 也可上传，但有数量限制。注册后在网站获取。")
                            .weak()
                            .size(12.0),
                    );
                }
            }
```

**注意**：打开浏览器需要 `open` crate。在 Cargo.toml 添加：

```toml
open = "5"
```

- [ ] **步骤 3：更新保存逻辑**

在"保存"按钮点击时，同时保存图片设置：

```rust
                if ui.button("保存").clicked() {
                    // CSS
                    app_config.custom_css = if new_css.trim().is_empty() {
                        None
                    } else {
                        Some(new_css.clone())
                    };
                    // 图片设置
                    app_config.image_hosting.default_strategy = match dialog.strategy_buffer {
                        1 => ImageStrategy::Base64,
                        2 => ImageStrategy::SmMs,
                        _ => ImageStrategy::Local,
                    };
                    app_config.image_hosting.local_dir = dialog.local_dir_buffer.clone();
                    app_config.image_hosting.smms.api_token = dialog.smms_token_buffer.clone();

                    if let Err(e) = app_config.save() {
                        tracing::error!("配置保存失败: {e}");
                    } else {
                        tracing::info!("配置已保存");
                    }
                    close_this = true;
                }
```

- [ ] **步骤 4：更新 open_dialog 调用方**

修改 `menu.rs` 中打开设置的调用：
```rust
if ui.button("设置...").clicked() {
    settings_dialog.open_dialog(app_config.custom_css.as_deref(), &app_config.image_hosting);
    ui.close();
}
```

需要将 `app_config` 改为 `&AppConfig`（已是），直接调用即可。

- [ ] **步骤 5：更新测试**

修改 `crates/zdown-app/src/settings_dialog.rs` 的测试：

```rust
#[test]
fn open_populates_buffer() {
    let mut dialog = SettingsDialog::default();
    dialog.open_dialog(Some("h1{color:red}"), &Default::default());
    assert!(dialog.open);
    assert_eq!(dialog.css_buffer, "h1{color:red}");
    assert_eq!(dialog.local_dir_buffer, "images");
}
```

- [ ] **步骤 6：编译验证**

```bash
cargo build -p zdown-app
```

预期：编译成功

- [ ] **步骤 7：运行测试**

```bash
cargo test -p zdown-app -- settings_dialog
```

预期：测试 PASS

- [ ] **步骤 8：Commit**

```bash
git add crates/zdown-app/src/settings_dialog.rs crates/zdown-app/src/menu.rs crates/zdown-app/Cargo.toml && git commit -m "feat(zdown-app): add image hosting settings tab with strategy/local dir/SM.MS token"
```

---

### 任务 8：全量验证

- [ ] **步骤 1：fmt**

```bash
cargo fmt
```

- [ ] **步骤 2：clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **步骤 3：test**

```bash
cargo test --workspace
```

- [ ] **步骤 4：如有 warning/error，修复并重新运行直到 clean**

- [ ] **步骤 5：最终 Commit**

```bash
git add -A && git commit -m "chore: fmt + clippy fix for image hosting feature"
```

---

### 验收检查清单

- [ ] `cargo fmt` clean
- [ ] `cargo clippy --all-targets` 0 warnings
- [ ] `cargo test --workspace` 全部通过
- [ ] 拖拽 PNG/JPEG 文件到编辑器 → `![alt](url)` 出现在光标位置
- [ ] 粘贴剪贴板图片 → 同上
- [ ] Ctrl+I 浏览选择图片 → 同上
- [ ] 切换存储策略为 Base64 → 插入图片生成 data URI
- [ ] 切换存储策略为 SM.MS → 插入图片上传到云端
- [ ] 配置持久化：修改设置 → 重启 → 设置保留
- [ ] 降级：无网络时 SM.MS 上传失败显示错误信息，不 panic
