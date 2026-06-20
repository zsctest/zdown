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
            inner: LruCache::new(
                std::num::NonZeroUsize::new(cap.max(1)).unwrap_or(std::num::NonZeroUsize::MIN),
            ),
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
    format!("{:x}", hasher.finalize())
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
