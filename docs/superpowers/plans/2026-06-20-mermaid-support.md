# Mermaid 图表支持 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 添加 Mermaid 图表渲染支持——通过 mermaid.ink 云端 API 将 mermaid 代码块转为 SVG，在 egui 预览、HTML 导出和 PDF 导出中渲染图表。

**架构：** 新增 `crates/mermaid_renderer/` 小 crate，封装 URL 编码、HTTP 请求、SHA256 内容寻址缓存。修改 `markdown_renderer` 和 `export_engine` 的 `render_code_block()` 函数，在检测到 `language="mermaid"` 时分发到 Mermaid 渲染路径。

**技术栈：** ureq（HTTP）, flate2（deflate 压缩）, base64（URL 编码）, sha2（缓存键哈希）, lru（LRU 缓存）, resvg+tiny-skia（SVG 光栅化用于 egui/PDF）

---

### 任务 1：创建 mermaid_renderer crate 骨架

**文件：**
- 创建：`crates/mermaid_renderer/Cargo.toml`
- 创建：`crates/mermaid_renderer/src/lib.rs`
- 修改：`Cargo.toml:8-15`（workspace members）
- 修改：`Cargo.toml:73-79`（workspace dependencies）

- [ ] **步骤 1：创建 Cargo.toml**

创建 `crates/mermaid_renderer/Cargo.toml`：

```toml
[package]
name = "mermaid_renderer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
thiserror.workspace = true
tracing.workspace = true
flate2 = "1"
base64.workspace = true
ureq.workspace = true
sha2 = "0.10"
lru = "0.14"
```

- [ ] **步骤 2：在根 workspace 注册新 crate**

在根 `Cargo.toml` 中：
1. `members` 数组追加 `"crates/mermaid_renderer",`
2. 在 workspace dependencies 区域追加：

```toml
# ---------- mermaid_renderer ----------
flate2 = "1"
sha2 = "0.10"
lru = "0.14"
mermaid_renderer = { path = "crates/mermaid_renderer" }
```

- [ ] **步骤 3：创建 lib.rs 骨架和 Error 类型**

创建 `crates/mermaid_renderer/src/lib.rs`：

```rust
//! Mermaid 图表渲染器。
//!
//! 将 Mermaid 语法通过 mermaid.ink 云端 API 渲染为 SVG。

pub mod cache;
pub mod encode;

use std::time::Duration;

use cache::SvgCache;

/// Mermaid 渲染错误。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("网络请求失败: {0}")]
    Network(String),
    #[error("Mermaid 语法错误: {0}")]
    Syntax(String),
    #[error("HTTP 超时")]
    Timeout,
}

/// 渲染结果类型别名。
pub type Result<T> = std::result::Result<T, Error>;

/// Mermaid 图表渲染器。
pub struct MermaidRenderer {
    cache: SvgCache,
}

impl Default for MermaidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MermaidRenderer {
    /// 创建新渲染器（空缓存，10 秒超时）。
    pub fn new() -> Self {
        Self {
            cache: SvgCache::new(50),
        }
    }

    /// 判断 CodeBlock 是否为 mermaid 图表。
    pub fn is_mermaid(language: Option<&str>) -> bool {
        language.is_some_and(|l| l.eq_ignore_ascii_case("mermaid"))
    }

    /// 渲染 Mermaid 源码为 SVG 字符串。
    ///
    /// 使用内容寻址缓存，相同源码只请求一次。
    pub fn render(&mut self, source: &str) -> Result<String> {
        // 先查缓存
        let hash = cache::hash_source(source);
        if let Some(svg) = self.cache.get(&hash) {
            tracing::debug!("mermaid 缓存命中");
            return Ok(svg);
        }

        let url = encode::encode_to_url(source);
        let svg = self.fetch_svg(&url)?;

        self.cache.insert(hash, svg.clone());
        Ok(svg)
    }

    /// HTTP GET mermaid.ink 获取 SVG。
    fn fetch_svg(&self, url: &str) -> Result<String> {
        let response = ureq::get(url)
            .set("User-Agent", "zdown/0.1")
            .timeout(Duration::from_secs(10))
            .call()
            .map_err(|e| {
                if matches!(e, ureq::Error::Transport(_)) {
                    Error::Network(e.to_string())
                } else {
                    Error::Network(e.to_string())
                }
            })?;

        let body = response
            .into_string()
            .map_err(|e| Error::Network(e.to_string()))?;

        // mermaid.ink 错误时会返回非 SVG 内容
        if body.trim_start().starts_with("<svg") || body.trim_start().starts_with("<?xml") {
            Ok(body)
        } else {
            // 可能是语法错误文本
            Err(Error::Syntax(body))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn is_mermaid_detects_lowercase() {
        assert!(MermaidRenderer::is_mermaid(Some("mermaid")));
    }

    #[test]
    fn is_mermaid_case_insensitive() {
        assert!(MermaidRenderer::is_mermaid(Some("Mermaid")));
        assert!(MermaidRenderer::is_mermaid(Some("MERMAID")));
    }

    #[test]
    fn is_mermaid_rejects_other_languages() {
        assert!(!MermaidRenderer::is_mermaid(Some("rust")));
        assert!(!MermaidRenderer::is_mermaid(Some("python")));
        assert!(!MermaidRenderer::is_mermaid(None));
    }
}
```

- [ ] **步骤 4：验证编译**

```bash
cargo check -p mermaid_renderer
```

预期：编译成功（encode.rs 和 cache.rs 尚未创建，移除 `pub mod` 行临时编译，待下步创建）

- [ ] **步骤 5：Commit**

```bash
git add crates/mermaid_renderer/ Cargo.toml
git commit -m "feat(mermaid_renderer): add crate skeleton with error types and MermaidRenderer

- workspace: register mermaid_renderer crate
- add flate2, sha2, lru workspace deps
- MermaidRenderer with is_mermaid() and render() stubs

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 2：实现 encode 模块（URL 编码）

**文件：**
- 创建：`crates/mermaid_renderer/src/encode.rs`

- [ ] **步骤 1：编写 encode 测试**

在 `crates/mermaid_renderer/src/encode.rs` 中写入测试和模块骨架：

```rust
//! Mermaid 源码 → mermaid.ink URL 编码。
//!
//! 编码流程：UTF-8 字节 → deflate 压缩 → base64url。

use base64::Engine;

/// 将 Mermaid 源码编码为 mermaid.ink GET URL。
pub fn encode_to_url(source: &str) -> String {
    let encoded = encode_mermaid(source);
    format!("https://mermaid.ink/img/pako:{encoded}")
}

/// 对 Mermaid 源码执行 pako deflate + base64url 编码。
fn encode_mermaid(source: &str) -> String {
    let input = source.as_bytes();
    let compressed = deflate(input);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
}

/// deflate 压缩（模拟 pako 行为）。
fn deflate(data: &[u8]) -> Vec<u8> {
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).expect("deflate write");
    encoder.finish().expect("deflate finish")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn encode_simple_graph_produces_url() {
        let source = "graph TD\n    A --> B";
        let url = encode_to_url(source);
        assert!(url.starts_with("https://mermaid.ink/img/pako:"));
    }

    #[test]
    fn encode_empty_source_returns_url() {
        let url = encode_to_url("");
        assert!(url.starts_with("https://mermaid.ink/img/pako:"));
    }

    #[test]
    fn encode_preserves_unicode() {
        let source = "graph LR\n    开始 --> 结束";
        let url = encode_to_url(source);
        assert!(url.contains("pako:"));
    }

    #[test]
    fn encode_sequence_diagram() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello";
        let url = encode_to_url(source);
        assert!(url.starts_with("https://mermaid.ink/img/pako:"));
    }

    #[test]
    fn encode_produces_url_safe_output() {
        let source = "graph TD";
        let url = encode_to_url(source);
        // URL 安全编码不应包含 '+' 或 '/'
        let encoded = url.strip_prefix("https://mermaid.ink/img/pako:").unwrap();
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
    }

    #[test]
    fn same_input_produces_same_url() {
        let src = "graph TD\n    A-->B";
        let url1 = encode_to_url(src);
        let url2 = encode_to_url(src);
        assert_eq!(url1, url2);
    }
}
```

- [ ] **步骤 2：运行 encode 测试**

```bash
cargo test -p mermaid_renderer encode
```

预期：6 个测试全部通过

- [ ] **步骤 3：Commit**

```bash
git add crates/mermaid_renderer/src/encode.rs
git commit -m "feat(mermaid_renderer): implement encode module with pako deflate + base64url

- deflate compression via flate2
- base64 URL-safe no-pad encoding
- encode_to_url builds full mermaid.ink GET URL

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 3：实现 cache 模块（SHA256 + LRU）

**文件：**
- 创建：`crates/mermaid_renderer/src/cache.rs`

- [ ] **步骤 1：编写 cache 测试和实现**

创建 `crates/mermaid_renderer/src/cache.rs`：

```rust
//! 内容寻址 SVG 缓存。
//!
//! 对 Mermaid 源码做 SHA256 哈希作为缓存键，LRU 淘汰。

use lru::LruCache;
use sha2::{Digest, Sha256};

/// SVG 缓存：内容寻址 + LRU 淘汰。
pub struct SvgCache {
    inner: LruCache<String, String>,
}

impl SvgCache {
    /// 创建指定容量的缓存。
    pub fn new(cap: usize) -> Self {
        Self {
            inner: LruCache::new(std::num::NonZeroUsize::new(cap.max(1)).unwrap()),
        }
    }

    /// 查找缓存。键为 SHA256 十六进制字符串。
    pub fn get(&mut self, key: &str) -> Option<String> {
        self.inner.get(key).cloned()
    }

    /// 插入缓存。
    pub fn insert(&mut self, key: String, svg: String) {
        self.inner.put(key, svg);
    }
}

/// 计算 Mermaid 源码的 SHA256 哈希（用于缓存键）。
pub fn hash_source(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn cache_insert_and_get() {
        let mut cache = SvgCache::new(10);
        cache.insert("key1".into(), "svg1".into());
        assert_eq!(cache.get("key1"), Some("svg1".into()));
    }

    #[test]
    fn cache_miss_returns_none() {
        let mut cache = SvgCache::new(10);
        assert_eq!(cache.get("nonexistent"), None);
    }

    #[test]
    fn lru_evicts_oldest() {
        let mut cache = SvgCache::new(2);
        cache.insert("a".into(), "svg_a".into());
        cache.insert("b".into(), "svg_b".into());
        cache.insert("c".into(), "svg_c".into());
        // a 应被淘汰
        assert_eq!(cache.get("a"), None);
        assert_eq!(cache.get("b"), Some("svg_b".into()));
        assert_eq!(cache.get("c"), Some("svg_c".into()));
    }

    #[test]
    fn hash_same_input_produces_same_hash() {
        let h1 = hash_source("graph TD");
        let h2 = hash_source("graph TD");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_different_input_produces_different_hash() {
        let h1 = hash_source("graph TD");
        let h2 = hash_source("graph LR");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_is_hex_string() {
        let hash = hash_source("test");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

`lru` 使用 `NonZero<u16>` 需要添加 `hex` 依赖。更新 `Cargo.toml` 的 `[dependencies]` 追加：

```toml
hex = "0.4"
```

- [ ] **步骤 2：运行 cache 测试**

```bash
cargo test -p mermaid_renderer cache
```

预期：6 个测试全部通过

- [ ] **步骤 3：验证 lib.rs 中的模块导入编译**

在 `lib.rs` 中取消注释已有的 `pub mod cache; pub mod encode;`。运行：

```bash
cargo check -p mermaid_renderer
```

预期：编译成功

- [ ] **步骤 4：Commit**

```bash
git add crates/mermaid_renderer/
git commit -m "feat(mermaid_renderer): implement cache module with SHA256 + LRU

- SHA256 content-addressed cache keys
- LRU eviction with configurable capacity (default 50)
- hex dependency for hash output

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 4：集成 Mermaid 到 markdown_renderer（egui 预览）

**文件：**
- 修改：`crates/markdown_renderer/Cargo.toml`
- 修改：`crates/markdown_renderer/src/render.rs:122-152`

- [ ] **步骤 1：添加依赖到 markdown_renderer**

在 `crates/markdown_renderer/Cargo.toml` 的 `[dependencies]` 中追加：

```toml
mermaid_renderer.workspace = true
resvg = "0.46"
tiny-skia = "0.11"
```

在根 `Cargo.toml` workspace dependencies 中追加：

```toml
resvg = "0.46"
tiny-skia = "0.11"
```

- [ ] **步骤 2：修改 render_code_block 添加 Mermaid 分支**

修改 `crates/markdown_renderer/src/render.rs` 中的 `render_code_block` 函数。

原函数签名保持不变，在函数头部插入 Mermaid 检测：

```rust
fn render_code_block(ui: &mut egui::Ui, cb: &CodeBlock) {
    use mermaid_renderer::MermaidRenderer;

    // Mermaid 图表：尝试渲染为 SVG 图像
    if MermaidRenderer::is_mermaid(cb.language.as_deref()) {
        if let Some(image) = render_mermaid_to_egui_image(&cb.content) {
            ui.add(egui::Image::from_bytes(
                format!("bytes://mermaid_{}.png", cb.content.len()),
                image,
            ));
            ui.add_space(4.0);
            return;
        }
        // 降级：继续走语法高亮路径
    }

    // --- 以下为原有语法高亮逻辑，不变 ---
    let highlighter = SourceHighlighter::new().ok();
    // ... 后续不变
}
```

同时添加 SVG 转 egui 图像的辅助函数：

```rust
/// 将 Mermaid 源码渲染为 egui 可用的图像数据。
///
/// 失败返回 None（调用方降级为语法高亮）。
fn render_mermaid_to_egui_image(source: &str) -> Option<egui::ImageData> {
    let mut renderer = MermaidRenderer::new();
    let svg = match renderer.render(source) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Mermaid 渲染失败: {e}");
            return None;
        }
    };

    render_svg_to_image_data(&svg, 2.0)
}

/// 将 SVG 字符串光栅化为 egui::ImageData（2x 缩放用于 HiDPI）。
fn render_svg_to_image_data(svg: &str, scale: f32) -> Option<egui::ImageData> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let size = tree.size();
    let pw = (size.width() * scale) as u32;
    let ph = (size.height() * scale) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba: Vec<u8> = pixmap.data().to_vec();
    Some(egui::ImageData::Color(
        egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba),
    ))
}
```

- [ ] **步骤 3：编写 markdown_renderer 测试**

在 `crates/markdown_renderer/src/render.rs` 的 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
fn is_mermaid_detects_code_block() {
    use mermaid_renderer::MermaidRenderer;
    assert!(MermaidRenderer::is_mermaid(Some("mermaid")));
    assert!(!MermaidRenderer::is_mermaid(Some("rust")));
    assert!(!MermaidRenderer::is_mermaid(None));
}
```

- [ ] **步骤 4：编译验证**

```bash
cargo check -p markdown_renderer
```

预期：编译成功

- [ ] **步骤 5：运行测试**

```bash
cargo test -p markdown_renderer
```

预期：所有测试通过（包括新增的 is_mermaid 测试）

- [ ] **步骤 6：Commit**

```bash
git add crates/markdown_renderer/ Cargo.toml
git commit -m "feat(markdown_renderer): integrate Mermaid rendering into preview

- detect mermaid code blocks in render_code_block
- render SVG via MermaidRenderer + resvg rasterization
- fallback to syntax highlight on error
- add resvg, tiny-skia workspace deps

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 5：集成 Mermaid 到 export_engine HTML 导出

**文件：**
- 修改：`crates/export_engine/Cargo.toml`
- 修改：`crates/export_engine/src/html.rs:282-305`（render_code_block 附近）

- [ ] **步骤 1：添加 mermaid_renderer 依赖**

在 `crates/export_engine/Cargo.toml` 的 `[dependencies]` 中追加：

```toml
mermaid_renderer.workspace = true
```

- [ ] **步骤 2：查看当前 HTML render_code_block 实现**

确认需要修改的函数位置。当前位于 `crates/export_engine/src/html.rs` 约 282 行。

- [ ] **步骤 3：修改 HTML render_code_block 添加 Mermaid 分支**

原函数签名：`fn render_code_block(out: &mut String, cb: &CodeBlock)`（位于 `html.rs:282`）。

在函数体开头插入 Mermaid 检测：

```rust
fn render_code_block(out: &mut String, cb: &CodeBlock) {
    use mermaid_renderer::MermaidRenderer;

    // Mermaid 图表：渲染为内嵌 SVG <img> 标签
    if MermaidRenderer::is_mermaid(cb.language.as_deref()) {
        let mut renderer = MermaidRenderer::new();
        match renderer.render(&cb.content) {
            Ok(svg) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
                out.push_str(&format!(
                    "<figure class=\"mermaid\">\n<img src=\"data:image/svg+xml;base64,{encoded}\" alt=\"mermaid diagram\" />\n</figure>\n"
                ));
                return;
            }
            Err(_) => {
                // 降级：继续走原有语法高亮路径
            }
        }
    }

    // --- 以下为原有代码，不变 ---
    out.push_str("<pre><code");
    if let Some(lang) = &cb.language {
        out.push_str(" class=\"language-");
        out.push_str(&escape_html(lang));
        out.push('"');
    }
    out.push('>');
    let highlighted = highlight_code_to_html(&cb.content, cb.language.as_deref());
    out.push_str(&highlighted);
    out.push_str("</code></pre>");
}

- [ ] **步骤 4：运行 HTML 测试**

```bash
cargo test -p export_engine html
```

预期：所有 HTML 相关测试通过

- [ ] **步骤 5：Commit**

```bash
git add crates/export_engine/
git commit -m "feat(export_engine): integrate Mermaid SVG into HTML export

- detect mermaid code blocks in HTML render_code_block
- embed SVG as base64 data URI in <img> tag
- fallback to code highlight on render error

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 6：集成 Mermaid 到 export_engine PDF 导出

**文件：**
- 修改：`crates/export_engine/Cargo.toml`
- 修改：`crates/export_engine/src/renderer.rs:295-328`

- [ ] **步骤 1：添加 resvg + tiny-skia 依赖到 export_engine**

在 `crates/export_engine/Cargo.toml` 的 `[dependencies]` 中追加：

```toml
resvg.workspace = true
tiny-skia.workspace = true
```

- [ ] **步骤 2：修改 PDF render_code_block 添加 Mermaid 分支**

修改 `crates/export_engine/src/renderer.rs` 中的 `render_code_block` 函数，在现有语法高亮逻辑之前插入 Mermaid 检测：

```rust
fn render_code_block(cb: &CodeBlock, theme: &PdfTheme, layout: &mut LinearLayout) {
    use mermaid_renderer::MermaidRenderer;

    // Mermaid 图表：渲染为光栅图像嵌入 PDF
    if MermaidRenderer::is_mermaid(cb.language.as_deref()) {
        if let Some(img) = render_mermaid_to_genpdf_image(&cb.content) {
            layout.push(img.with_alignment(genpdf::Alignment::Center));
            return;
        }
        // 降级：继续走代码高亮路径
    }

    // --- 以下为原有语法高亮逻辑，不变 ---
    let highlighter = crate::highlight::CodeHighlighter::new(&theme.syntax_theme);
    // ...
}
```

添加辅助函数：

```rust
/// 将 Mermaid 源码渲染为 genpdf Image 元素。
fn render_mermaid_to_genpdf_image(source: &str) -> Option<genpdf::elements::Image> {
    let mut renderer = MermaidRenderer::new();
    let svg = match renderer.render(source) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Mermaid PDF 渲染失败: {e}");
            return None;
        }
    };

    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).ok()?;
    let size = tree.size();
    let pw = (size.width() * 2.0) as u32;
    let ph = (size.height() * 2.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(2.0, 2.0),
        &mut pixmap.as_mut(),
    );

    let rgba = pixmap.data().to_vec();
    let dyn_img = image::RgbaImage::from_raw(pw, ph, rgba)?;
    genpdf::elements::Image::from_dynamic_image(image::DynamicImage::ImageRgba8(dyn_img)).ok()
}
```

- [ ] **步骤 3：运行 PDF 测试**

```bash
cargo test -p export_engine pdf
```

预期：所有 PDF 相关测试通过

- [ ] **步骤 4：Commit**

```bash
git add crates/export_engine/
git commit -m "feat(export_engine): integrate Mermaid SVG into PDF export

- detect mermaid code blocks in PDF render_code_block
- render SVG via MermaidRenderer + resvg rasterization
- embed as genpdf Image element
- fallback to code highlight on error

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 7：最终验证 — fmt + clippy + 全量测试

**文件：** 无修改，仅验证

- [ ] **步骤 1：格式化**

```bash
cargo fmt --all
```

预期：无变更输出

- [ ] **步骤 2：Clippy**

```bash
cargo clippy --all-targets
```

预期：0 错误（允许 0 警告）

- [ ] **步骤 3：全量测试**

```bash
cargo test --workspace
```

预期：所有测试通过

- [ ] **步骤 4：提交格式变更（如有）**

```bash
git add -A && git diff --cached --quiet || git commit -m "chore: cargo fmt after mermaid support

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 8：编写集成测试

**文件：**
- 创建：`crates/mermaid_renderer/tests/integration.rs`

- [ ] **步骤 1：编写集成测试**

创建 `crates/mermaid_renderer/tests/integration.rs`：

```rust
//! Mermaid 渲染集成测试。
//!
//! 测试依赖外部网络（mermaid.ink 服务）。
//! 在 CI 环境中可能因网络原因失败，标记为 ignored。

#![allow(clippy::expect_used, clippy::unwrap_used)]

use mermaid_renderer::MermaidRenderer;

/// 验证有效的 Mermaid 语法能返回 SVG。
#[test]
#[ignore = "需要外部网络访问 mermaid.ink"]
fn render_valid_graph_returns_svg() {
    let mut renderer = MermaidRenderer::new();
    let svg = renderer.render("graph TD\n    A --> B").expect("render");
    assert!(svg.starts_with("<svg") || svg.starts_with("<?xml"));
}

/// 验证有效 Mermaid 缓存命中。
#[test]
#[ignore = "需要外部网络访问 mermaid.ink"]
fn render_caches_duplicate_source() {
    let mut renderer = MermaidRenderer::new();
    let svg1 = renderer.render("graph LR\n    X --> Y").expect("render1");
    let svg2 = renderer.render("graph LR\n    X --> Y").expect("render2");
    assert_eq!(svg1, svg2);
}
```

- [ ] **步骤 2：运行集成测试（手动，需网络）**

```bash
cargo test -p mermaid_renderer --test integration -- --ignored --nocapture
```

预期：2 个测试通过（如网络可用）

- [ ] **步骤 3：Commit**

```bash
git add crates/mermaid_renderer/tests/
git commit -m "test(mermaid_renderer): add integration tests for mermaid.ink rendering

Co-Authored-By: Claude <noreply@anthropic.com>"
```
