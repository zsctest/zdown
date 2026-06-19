//! zdown 配置持久化模块。
//!
//! 存储用户偏好设置（自定义 CSS 等）到 TOML 文件。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 配置加载/保存过程中可能发生的错误。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML 反序列化错误: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("TOML 序列化错误: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("无法确定配置目录")]
    NoConfigDir,
}

/// zdown 应用配置。
///
/// `#[serde(default)]` 确保向后兼容：旧版本配置文件
/// 缺少新增字段时自动使用 Default 值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 自定义 CSS，追加到 HTML 导出的内置样式之后。
    /// `None` 表示不添加自定义样式。
    pub custom_css: Option<String>,
}

impl AppConfig {
    /// 从默认路径加载配置。首次运行时文件不存在则返回默认值。
    pub fn load() -> Result<Self, Error> {
        match Self::default_path() {
            Some(path) => Self::load_from(&path),
            None => Err(Error::NoConfigDir),
        }
    }

    /// 保存配置到默认路径。会自动创建父目录。
    pub fn save(&self) -> Result<(), Error> {
        match Self::default_path() {
            Some(path) => self.save_to(&path),
            None => Err(Error::NoConfigDir),
        }
    }

    /// 从指定路径加载配置。
    pub fn load_from(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// 保存配置到指定路径。会自动创建父目录。
    pub fn save_to(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 返回默认配置文件路径。
    ///
    /// Windows: `%APPDATA%/zdown/config.toml`
    /// Linux:   `$XDG_CONFIG_HOME/zdown/config.toml` 或 `~/.config/zdown/config.toml`
    /// macOS:   `~/Library/Application Support/zdown/config.toml`
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("zdown").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("zdown_test_{name}.toml"))
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn default_config_has_no_css() {
        let config = AppConfig::default();
        assert!(config.custom_css.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = temp_path("roundtrip");
        cleanup(&path);

        let config = AppConfig {
            custom_css: Some("body { color: red; }".into()),
        };
        config.save_to(&path).expect("save_to");
        let loaded = AppConfig::load_from(&path).expect("load_from");
        assert_eq!(loaded.custom_css, Some("body { color: red; }".into()));
        cleanup(&path);
    }

    #[test]
    fn save_and_load_roundtrip_no_css() {
        let path = temp_path("no_css");
        cleanup(&path);

        let config = AppConfig::default();
        config.save_to(&path).expect("save_to");
        let loaded = AppConfig::load_from(&path).expect("load_from");
        assert!(loaded.custom_css.is_none());
        cleanup(&path);
    }

    #[test]
    fn load_nonexistent_returns_default() {
        let path = temp_path("nonexistent_xyz");
        cleanup(&path);

        let config = AppConfig::load_from(&path).expect("load_from");
        assert!(config.custom_css.is_none());
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = std::env::temp_dir().join("zdown_test_nested").join("sub");
        let path = dir.join("config.toml");
        let _ = std::fs::remove_dir_all(&dir);

        let config = AppConfig::default();
        config.save_to(&path).expect("save_to");
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn serialize_produces_valid_toml() {
        let config = AppConfig {
            custom_css: Some("h1 { font-size: 2em; }".into()),
        };
        let toml_str = toml::to_string_pretty(&config).expect("serialize");
        let parsed: AppConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(parsed.custom_css.as_deref(), Some("h1 { font-size: 2em; }"));
    }

    #[test]
    fn default_path_returns_some() {
        let _ = AppConfig::default_path();
    }

    #[test]
    fn error_display() {
        let e = Error::NoConfigDir;
        assert!(e.to_string().contains("配置目录"));
    }
}
