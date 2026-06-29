//! 图片加载缓存模块。
//!
//! 负责从 data URI、远程 URL 或本地路径加载图片，
//! 并将结果缓存为 `Arc<egui::ColorImage>`，避免逐帧重复下载。
//!
//! - data URI / 本地文件：同步加载（毫秒级）。
//! - 远程 URL：后台线程加载，不阻塞 UI；完成后通过 `poll_pending` 收集结果
//!   并自动 `request_repaint`。
//!
//! LRU 上限 20 条，超出丢弃最旧。失败 URL 记录在独立集合中，
//! 会随 LRU 淘汰一起清理。

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 后台加载任务的共享槽位：下载线程写入结果，主线程通过 `try_lock` 轮询。
type PendingSlot = Arc<Mutex<Option<Result<egui::ColorImage, String>>>>;

/// 图片缓存。key 为 URL，value 为已加载的 egui 图片数据。
/// 失败 URL 记录在 `failed` 集合中，避免重复重试。
///
/// 同时缓存 `TextureHandle`，避免每帧调用 `load_texture` 触发
/// egui 0.34 的纹理替换导致图片闪烁。
///
/// **重要**：必须存储 `TextureHandle` 而非 `TextureId`。
/// `TextureHandle::Drop` 会调用 `TextureManager::free(id)` 释放纹理；
/// 只有持有 handle 才能保持纹理存活。
///
/// 远程图片通过后台线程加载，`poll_pending` 负责收尾。
pub struct ImageCache {
    images: VecDeque<(String, Arc<egui::ColorImage>)>,
    /// 缓存已注册的纹理句柄（key 为 URL）。
    /// 持有 handle 可防止纹理被 egui 释放。
    texture_handles: HashMap<String, egui::TextureHandle>,
    failed: HashSet<String>,
    max_entries: usize,
    /// 正在通过后台线程加载的远程图片。
    pending: HashMap<String, PendingSlot>,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self {
            images: VecDeque::new(),
            texture_handles: HashMap::new(),
            failed: HashSet::new(),
            max_entries: 20,
            pending: HashMap::new(),
        }
    }
}

impl ImageCache {
    /// 创建空缓存（上限 20 条）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建指定上限的缓存（测试用）。
    #[cfg(test)]
    pub fn with_max(max_entries: usize) -> Self {
        Self {
            images: VecDeque::new(),
            texture_handles: HashMap::new(),
            failed: HashSet::new(),
            max_entries,
            pending: HashMap::new(),
        }
    }

    /// 获取或加载图片。
    ///
    /// - 缓存命中 / 失败过 → 立即返回
    /// - 远程 URL → 后台线程加载，返回 `None`（等 `poll_pending` 收尾）
    /// - data URI / 本地文件 → 同步加载（毫秒级，不阻塞感知）
    ///
    /// `working_dir` 用于解析本地相对路径，仅对本地文件 URL 有效。
    pub fn get_or_load(
        &mut self,
        url: &str,
        working_dir: Option<&Path>,
    ) -> Option<Arc<egui::ColorImage>> {
        // 命中缓存 → LRU 提升
        if let Some(pos) = self.images.iter().position(|(k, _)| k == url) {
            let (key, img) = self.images.remove(pos)?;
            self.images.push_front((key, Arc::clone(&img)));
            return Some(img);
        }
        // 已经失败过
        if self.failed.contains(url) {
            return None;
        }
        // 正在后台加载
        if self.pending.contains_key(url) {
            return None;
        }
        // 远程 URL → 后台线程加载，避免阻塞 UI
        if url.starts_with("http://") || url.starts_with("https://") {
            let url_owned = url.to_owned();
            // ponyail: one thread per image, pool only if >20 concurrent images
            let slot: PendingSlot = Arc::new(Mutex::new(None));
            let slot_clone = Arc::clone(&slot);
            std::thread::spawn(move || {
                let result = load_from_remote(&url_owned);
                // 写入失败无害：最坏情况下 poll_pending 下一帧重试
                if let Ok(mut guard) = slot_clone.lock() {
                    *guard = Some(result);
                }
            });
            self.pending.insert(url.to_owned(), slot);
            return None;
        }
        // data URI / 本地文件 → 同步加载
        match load_image(url, working_dir) {
            Ok(color_image) => {
                let arc = Arc::new(color_image);
                // LRU 淘汰
                while self.images.len() >= self.max_entries {
                    if let Some((old_key, _)) = self.images.pop_back() {
                        self.failed.remove(&old_key);
                        self.texture_handles.remove(&old_key);
                    }
                }
                self.images.push_front((url.to_owned(), Arc::clone(&arc)));
                Some(arc)
            }
            Err(e) => {
                tracing::warn!("图片加载失败 [{url}]: {e}");
                self.failed.insert(url.to_owned());
                None
            }
        }
    }

    /// 轮询后台加载结果，将已完成的移至缓存。
    ///
    /// 应在每帧渲染前调用。有结果时自动 `ctx.request_repaint()`。
    pub fn poll_pending(&mut self, ctx: &egui::Context) {
        // 快速路径：无 pending 直接返回
        if self.pending.is_empty() {
            return;
        }

        let mut completed: Vec<(String, Result<egui::ColorImage, String>)> = Vec::new();
        for (url, slot) in &self.pending {
            // try_lock 而非 lock：避免下载线程持锁时阻塞渲染
            if let Ok(mut guard) = slot.try_lock() {
                if let Some(result) = guard.take() {
                    completed.push((url.clone(), result));
                }
            }
        }

        if completed.is_empty() {
            return;
        }

        for (url, result) in completed {
            self.pending.remove(&url);
            match result {
                Ok(color_image) => {
                    let arc = Arc::new(color_image);
                    while self.images.len() >= self.max_entries {
                        if let Some((old_key, _)) = self.images.pop_back() {
                            self.failed.remove(&old_key);
                            self.texture_handles.remove(&old_key);
                        }
                    }
                    self.images.push_front((url.clone(), Arc::clone(&arc)));
                    tracing::debug!("远程图片加载完成 [{url}]");
                }
                Err(e) => {
                    tracing::warn!("图片加载失败 [{url}]: {e}");
                    self.failed.insert(url);
                }
            }
        }
        ctx.request_repaint();
    }

    /// 查询已缓存图片的原始像素尺寸（不触发加载）。
    /// ponytail: 仅遍历 images deque，remote 未加载完成返回 None。
    pub fn get_cached_dimensions(&self, url: &str) -> Option<[usize; 2]> {
        self.images
            .iter()
            .find(|(k, _)| k == url)
            .map(|(_, img)| img.size)
    }

    /// 获取缓存的纹理 ID（存在则返回 `Some`，否则 `None`）。
    ///
    /// ID 从存储的 `TextureHandle` 派生，handle 本身保持在缓存中
    /// 以阻止 egui 释放纹理。
    pub fn get_texture_id(&self, url: &str) -> Option<egui::TextureId> {
        self.texture_handles.get(url).map(|h| h.id())
    }

    /// 存储纹理句柄以保持纹理存活。
    ///
    /// **必须存储 `TextureHandle` 而非 `TextureId`**：
    /// `TextureHandle::Drop` 会释放纹理，只存储 ID 会导致
    /// 下一帧纹理丢失。
    pub fn register_texture_handle(&mut self, url: &str, handle: egui::TextureHandle) {
        self.texture_handles.insert(url.to_owned(), handle);
    }

    /// 清空缓存（切换文档时调用，避免旧文档的图片占用内存）。
    pub fn clear(&mut self) {
        self.images.clear();
        self.texture_handles.clear();
        self.failed.clear();
        // 后台线程仍持有 slot Arc，完成后自然丢弃
        self.pending.clear();
    }

    /// 当前缓存条目数（测试用）。
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// 缓存是否为空（测试用）。
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

// ---------------------------------------------------------------------------
// 内部加载函数
// ---------------------------------------------------------------------------

/// 根据 URL 类型分发到对应的加载函数。
fn load_image(url: &str, working_dir: Option<&Path>) -> Result<egui::ColorImage, String> {
    if url.starts_with("data:") {
        load_from_data_uri(url)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        load_from_remote(url)
    } else {
        load_from_local(url, working_dir)
    }
}

/// 加载 data URI 图片（`data:image/<type>;base64,<data>`）。
fn load_from_data_uri(url: &str) -> Result<egui::ColorImage, String> {
    let b64: Vec<&str> = url.splitn(2, ";base64,").collect();
    if b64.len() < 2 {
        return Err("无效 data URI 格式：缺少 ;base64,".into());
    }
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64[1])
        .map_err(|e| format!("base64 解码失败: {e}"))?;
    let dyn_img = image::load_from_memory(&bytes).map_err(|e| format!("图片解码失败: {e}"))?;
    dynamic_to_egui(dyn_img)
}

/// 加载远程 URL 图片（10 秒超时）。
fn load_from_remote(url: &str) -> Result<egui::ColorImage, String> {
    let response = ureq::get(url)
        .set("User-Agent", "zdown/0.1 (markdown-editor)")
        .set(
            "Accept",
            "image/avif,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5",
        )
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("远程请求失败 [{url}]: {e}"))?;

    let status = response.status();
    if status != 200 {
        return Err(format!("HTTP {} [{url}] — 非 200 状态码", status));
    }

    let content_type = response.header("Content-Type").unwrap_or("").to_lowercase();
    // 返回的不是图片类型则提前报错，给出清晰信息（常见于反盗链 / CDN 拦截）
    if !content_type.is_empty()
        && !content_type.starts_with("image/")
        && content_type != "application/octet-stream"
    {
        return Err(format!(
            "非图片 Content-Type [{url}]: {content_type} — 可能是反盗链或需要登录"
        ));
    }

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("读取远程响应失败 [{url}]: {e}"))?;

    if bytes.is_empty() {
        return Err(format!("空响应体 [{url}]"));
    }

    let dyn_img = image::load_from_memory(&bytes).map_err(|e| {
        let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(100)]);
        format!("图片解码失败 [{url}]: {e} — 响应前 100 字节: {preview}")
    })?;
    dynamic_to_egui(dyn_img)
}

/// 加载本地文件图片。
fn load_from_local(path_str: &str, working_dir: Option<&Path>) -> Result<egui::ColorImage, String> {
    let path = Path::new(path_str);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let wd = working_dir.ok_or_else(|| "相对路径图片需要设置 working_dir".to_owned())?;
        wd.join(path_str)
    };
    let dyn_img = image::open(&resolved).map_err(|e| format!("打开本地图片失败: {e}"))?;
    dynamic_to_egui(dyn_img)
}

/// 将 `image::DynamicImage` 转为 `egui::ColorImage`（RGBA 无预乘）。
fn dynamic_to_egui(img: image::DynamicImage) -> Result<egui::ColorImage, String> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let data = rgba.into_raw();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        &data,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    /// 从有效的 data URI 加载 1x1 红色 PNG。
    #[test]
    fn load_from_data_uri_success() {
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let uri = format!("data:image/png;base64,{b64}");
        let img = load_image(&uri, None).expect("应从有效 data URI 加载图片");
        assert_eq!(img.size, [1, 1]);
    }

    /// 无效 base64 返回错误。
    #[test]
    fn load_from_data_uri_invalid_base64() {
        let uri = "data:image/png;base64,!!!not-valid-base64!!!";
        assert!(load_image(uri, None).is_err());
    }

    /// 不存在的本地文件返回错误。
    #[test]
    fn load_from_local_nonexistent() {
        let nonexistent = std::env::temp_dir().join("__zdown_nonexistent_test__.png");
        let path_str = nonexistent.to_str().expect("临时路径不是有效 UTF-8");
        assert!(load_image(path_str, None).is_err());
    }

    /// 相对路径但未提供 working_dir 返回错误。
    #[test]
    fn load_from_local_relative_without_working_dir() {
        assert!(load_image("relative/image.png", None).is_err());
    }

    /// 缓存命中测试（含 LRU 提升）。
    #[test]
    fn cache_hit_and_lru_promotion() {
        let mut cache = ImageCache::new();
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let uri = format!("data:image/png;base64,{b64}");
        let first = cache.get_or_load(&uri, None);
        assert!(first.is_some());
        // 第二次应从缓存命中
        let second = cache.get_or_load(&uri, None);
        assert!(second.is_some());
        assert_eq!(cache.len(), 1);
    }

    /// 失败 URL 不会被重复加载。
    #[test]
    fn cache_failed() {
        let mut cache = ImageCache::new();
        let uri = "data:image/png;base64,!!!bad!!!";
        assert!(cache.get_or_load(uri, None).is_none());
        assert!(cache.failed.contains(uri));
        // 第二次直接返回 None，不重试
        assert!(cache.get_or_load(uri, None).is_none());
    }

    /// clear 清空缓存。
    #[test]
    fn cache_clear() {
        let mut cache = ImageCache::with_max(5);
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let uri = format!("data:image/png;base64,{b64}");
        cache.get_or_load(&uri, None);
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.failed.is_empty());
    }

    /// LRU 淘汰：超出上限时最旧条目被移除。
    /// 通过创建临时本地文件（同一 1x1 PNG 内容但不同路径 = 不同缓存键）测试。
    #[test]
    fn lru_eviction() {
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let png_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .expect("decode 1x1 png");

        let dir = std::env::temp_dir().join("__zdown_lru_test__");
        let _ = std::fs::create_dir(&dir);
        let file_a = dir.join("a.png");
        let file_b = dir.join("b.png");
        let file_c = dir.join("c.png");
        let file_d = dir.join("d.png");

        for path in &[&file_a, &file_b, &file_c, &file_d] {
            std::fs::write(path, &png_bytes).expect("write png");
        }

        let mut cache = ImageCache::with_max(3);
        let wd = Some(dir.as_path());

        // 加载 a, b, c → 缓存满（3/3）
        assert!(cache.get_or_load(&file_a.to_string_lossy(), wd).is_some());
        assert!(cache.get_or_load(&file_b.to_string_lossy(), wd).is_some());
        assert!(cache.get_or_load(&file_c.to_string_lossy(), wd).is_some());
        assert_eq!(cache.len(), 3);

        // 加载 d → 触发淘汰，最旧的 a 被移除
        assert!(cache.get_or_load(&file_d.to_string_lossy(), wd).is_some());
        assert_eq!(cache.len(), 3);

        // 重新访问 a → 应重新加载（已被淘汰），但 b 或 c 可能被淘汰
        assert!(cache.get_or_load(&file_a.to_string_lossy(), wd).is_some());
        assert_eq!(cache.len(), 3);

        // 清理
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 同一 URL 多次访问只占用一个缓存槽位。
    #[test]
    fn same_url_single_slot() {
        let mut cache = ImageCache::with_max(10);
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let uri = format!("data:image/png;base64,{b64}");
        cache.get_or_load(&uri, None);
        cache.get_or_load(&uri, None);
        cache.get_or_load(&uri, None);
        assert_eq!(cache.len(), 1, "同一 URL 不应重复缓存");
    }
}
