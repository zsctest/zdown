//! 最近文件列表，TOML 持久化到 <config_dir>/zdown/recent.toml。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// 最多保留多少条最近文件。
const MAX_ENTRIES: usize = 10;

#[derive(Serialize, Deserialize)]
struct RecentFileList {
    paths: Vec<String>,
}

/// 最近文件管理器。
pub struct RecentFiles {
    paths: Vec<PathBuf>,
    storage_path: PathBuf,
}

impl RecentFiles {
    /// 从默认路径加载（`<config_dir>/zdown/recent.toml`）。
    /// 文件不存在或解析失败时返回空列表。
    pub fn load() -> Self {
        let storage_path = default_storage_path();
        Self::load_from(&storage_path)
    }

    /// 从指定路径加载（测试用）。
    pub fn load_from(storage_path: &Path) -> Self {
        match std::fs::read_to_string(storage_path) {
            Ok(content) => match toml::from_str::<RecentFileList>(&content) {
                Ok(list) => Self {
                    paths: list.paths.into_iter().map(PathBuf::from).collect(),
                    storage_path: storage_path.to_path_buf(),
                },
                Err(_) => Self::empty_at(storage_path),
            },
            Err(_) => Self::empty_at(storage_path),
        }
    }

    fn empty_at(storage_path: &Path) -> Self {
        Self {
            paths: Vec::new(),
            storage_path: storage_path.to_path_buf(),
        }
    }

    /// 构造空列表，用默认存储路径。
    pub fn new() -> Self {
        Self::empty_at(&default_storage_path())
    }

    /// 添加路径。按 canonicalize 去重并移到队首。超过上限丢弃最旧。
    pub fn add(&mut self, path: PathBuf) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        self.paths.retain(|p| {
            let p_canon = p.canonicalize().unwrap_or_else(|_| p.clone());
            p_canon != canonical
        });
        self.paths.insert(0, path);
        if self.paths.len() > MAX_ENTRIES {
            self.paths.truncate(MAX_ENTRIES);
        }
    }

    /// 当前最近文件列表（队首为最新）。
    pub fn list(&self) -> &[PathBuf] {
        &self.paths
    }

    /// 保存到存储路径。
    pub fn save(&self) -> Result<()> {
        let list = RecentFileList {
            paths: self
                .paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        };
        let toml_str = toml::to_string(&list).map_err(|e| Error::Dialog(e.to_string()))?;
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.storage_path, toml_str)?;
        Ok(())
    }

    /// 存储路径（测试用）。
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new()
    }
}

fn default_storage_path() -> PathBuf {
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config.join("zdown").join("recent.toml")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use tempfile::TempDir;

    fn temp_storage() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("recent.toml");
        (dir, path)
    }

    #[test]
    fn load_nonexistent_returns_empty() {
        let (_dir, path) = temp_storage();
        let rf = RecentFiles::load_from(&path);
        assert!(rf.list().is_empty());
    }

    #[test]
    fn add_one_appears_in_list() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        rf.add(PathBuf::from("/tmp/a.md"));
        assert_eq!(rf.list(), &[PathBuf::from("/tmp/a.md")]);
    }

    #[test]
    fn add_moves_existing_to_front() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        rf.add(PathBuf::from("/tmp/a.md"));
        rf.add(PathBuf::from("/tmp/b.md"));
        rf.add(PathBuf::from("/tmp/a.md"));
        assert_eq!(rf.list()[0], PathBuf::from("/tmp/a.md"));
        assert_eq!(rf.list().len(), 2);
    }

    #[test]
    fn add_dedupes_canonical() {
        let dir = TempDir::new().expect("tempdir");
        let real = dir.path().join("x.md");
        std::fs::write(&real, "").expect("write");
        let (_dir2, storage) = temp_storage();
        let mut rf = RecentFiles::load_from(&storage);
        rf.add(real.clone());
        rf.add(dir.path().join("x.md"));
        assert_eq!(rf.list().len(), 1);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        rf.add(PathBuf::from("/tmp/a.md"));
        rf.add(PathBuf::from("/tmp/b.md"));
        rf.save().expect("save");

        let reloaded = RecentFiles::load_from(&path);
        assert_eq!(reloaded.list(), rf.list());
    }

    #[test]
    fn max_entries_truncates() {
        let (_dir, path) = temp_storage();
        let mut rf = RecentFiles::load_from(&path);
        for i in 0..(MAX_ENTRIES + 5) {
            rf.add(PathBuf::from(format!("/tmp/file_{i}.md")));
        }
        assert_eq!(rf.list().len(), MAX_ENTRIES);
        assert_eq!(
            rf.list()[0],
            PathBuf::from(format!("/tmp/file_{}.md", MAX_ENTRIES + 4))
        );
    }

    #[test]
    fn save_creates_parent_dir() {
        let dir = TempDir::new().expect("tempdir");
        let nested = dir.path().join("nested").join("sub").join("recent.toml");
        let mut rf = RecentFiles::load_from(&nested);
        rf.add(PathBuf::from("/tmp/x.md"));
        rf.save().expect("save");
        assert!(nested.exists());
    }

    #[test]
    fn load_corrupt_returns_empty() {
        let (_dir, path) = temp_storage();
        std::fs::write(&path, "this is not toml: {{{").expect("write");
        let rf = RecentFiles::load_from(&path);
        assert!(rf.list().is_empty());
    }

    #[test]
    fn save_empty_list() {
        let (_dir, path) = temp_storage();
        let rf = RecentFiles::load_from(&path);
        rf.save().expect("save");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("paths"));
    }
}
