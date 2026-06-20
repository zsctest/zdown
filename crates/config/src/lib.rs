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

/// 主题模式。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// zdown 应用配置。
///
/// `#[serde(default)]` 确保向后兼容：旧版本配置文件
/// 缺少新增字段时自动使用 Default 值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// 自定义 CSS，追加到 HTML 导出的内置样式之后。
    /// `None` 表示不添加自定义样式。
    pub custom_css: Option<String>,

    /// UI 主题：暗色或亮色。默认暗色。
    pub theme: ThemeMode,
    /// 图片托管配置。
    pub image_hosting: ImageHostingConfig,

    /// 拼写检查开关。默认启用。
    #[serde(default = "default_spell_check")]
    pub spell_check_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            custom_css: None,
            theme: ThemeMode::Dark,
            image_hosting: ImageHostingConfig::default(),
            spell_check_enabled: true,
        }
    }
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

fn default_spell_check() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
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
            ..Default::default()
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
            ..Default::default()
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

    #[test]
    fn theme_mode_default_is_dark() {
        assert!(matches!(ThemeMode::default(), ThemeMode::Dark));
    }

    #[test]
    fn theme_mode_roundtrip() {
        let path = temp_path("theme");
        cleanup(&path);

        let config = AppConfig {
            custom_css: None,
            theme: ThemeMode::Light,
            ..Default::default()
        };
        config.save_to(&path).expect("save");
        let loaded = AppConfig::load_from(&path).expect("load");
        assert!(matches!(loaded.theme, ThemeMode::Light));
        cleanup(&path);
    }

    #[test]
    fn old_config_without_theme_defaults_to_dark() {
        let path = temp_path("old_config");
        cleanup(&path);
        // 写入一个只有 custom_css 字段的旧格式 TOML
        std::fs::write(&path, "custom_css = \"h1 { color: red; }\"\n").expect("write");
        let loaded = AppConfig::load_from(&path).expect("load");
        // theme 字段不存在 → 使用 serde(default) → ThemeMode::Dark
        assert!(matches!(loaded.theme, ThemeMode::Dark));
        assert_eq!(loaded.custom_css.as_deref(), Some("h1 { color: red; }"));
        cleanup(&path);
    }

    #[test]
    fn config_toml_contains_theme_field() {
        let config = AppConfig {
            custom_css: None,
            theme: ThemeMode::Light,
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&config).expect("serialize");
        assert!(
            toml_str.contains("theme"),
            "TOML 应包含 theme 字段: {}",
            toml_str
        );
        assert!(
            toml_str.contains("Light"),
            "TOML 应包含 Light: {}",
            toml_str
        );
    }

    // ── ImageHostingConfig 测试 ──

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
            ..Default::default()
        };
        config.save_to(&path).expect("save");
        let loaded = AppConfig::load_from(&path).expect("load");
        assert!(matches!(
            loaded.image_hosting.default_strategy,
            ImageStrategy::Base64
        ));
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
        assert!(matches!(
            loaded.image_hosting.default_strategy,
            ImageStrategy::Local
        ));
        assert_eq!(loaded.image_hosting.local_dir, "images");
        cleanup(&path);
    }

    #[test]
    fn spell_check_default_enabled() {
        let config = AppConfig::default();
        assert!(config.spell_check_enabled);
    }

    #[test]
    fn spell_check_deserialize_missing_field_defaults_true() {
        let toml_str = r#"
theme = "Dark"
"#;
        let config: AppConfig = toml::from_str(toml_str).expect("parse");
        assert!(config.spell_check_enabled);
    }

    #[test]
    fn spell_check_roundtrip() {
        let mut config = AppConfig::default();
        config.spell_check_enabled = false;
        let toml_str = toml::to_string_pretty(&config).expect("serialize");
        let restored: AppConfig = toml::from_str(&toml_str).expect("deserialize");
        assert!(!restored.spell_check_enabled);
    }
}
