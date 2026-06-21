//! zdown 多语言国际化模块。
//!
//! 基于 Fluent 实现运行时热切换的中英双语支持。

pub mod resource;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use std::collections::HashMap;

/// 支持的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Lang {
    /// 中文简体
    ZhCN,
    /// English (United States)
    EnUS,
}

impl Lang {
    /// 返回语言标签字符串（用于持久化到 config.toml）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZhCN => "zh-CN",
            Self::EnUS => "en-US",
        }
    }

    /// 返回用户可读的显示名。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ZhCN => "中文",
            Self::EnUS => "English",
        }
    }
}

impl std::str::FromStr for Lang {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "zh-CN" => Ok(Self::ZhCN),
            "en-US" => Ok(Self::EnUS),
            _ => Err("unknown language tag; expected 'zh-CN' or 'en-US'"),
        }
    }
}

/// 国际化管理器。
pub struct I18n {
    lang: Lang,
    bundles: HashMap<Lang, FluentBundle<FluentResource>>,
}

impl I18n {
    /// 创建实例，预加载所有语言的 FTL 资源。
    pub fn new() -> Self {
        let mut bundles = HashMap::new();
        bundles.insert(Lang::ZhCN, resource::create_bundle_zh_cn());
        bundles.insert(Lang::EnUS, resource::create_bundle_en_us());
        Self {
            lang: Lang::ZhCN,
            bundles,
        }
    }

    /// 以指定语言创建实例。
    pub fn with_lang(lang: Lang) -> Self {
        let mut slf = Self::new();
        slf.lang = lang;
        slf
    }

    /// 获取当前语言。
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// 切换语言（热切换）。
    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
    }

    /// 翻译指定 key，可选参数插值。
    pub fn tr(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let bundle = match self.bundles.get(&self.lang) {
            Some(b) => b,
            None => return key.to_string(),
        };
        let msg = match bundle.get_message(key) {
            Some(m) => m,
            None => return key.to_string(),
        };
        let pattern = match msg.value() {
            Some(p) => p,
            None => return key.to_string(),
        };
        let mut errors = vec![];
        let value = bundle.format_pattern(pattern, args, &mut errors);
        value.to_string()
    }

    /// 无参数翻译便捷方法。
    pub fn t(&self, key: &str) -> String {
        self.tr(key, None)
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use fluent_bundle::FluentArgs;

    #[test]
    fn new_creates_bundles() {
        let i18n = I18n::new();
        assert!(i18n.bundles.contains_key(&Lang::ZhCN));
        assert!(i18n.bundles.contains_key(&Lang::EnUS));
        assert_eq!(i18n.lang(), Lang::ZhCN);
    }

    #[test]
    fn with_lang_en_us() {
        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.lang(), Lang::EnUS);
    }

    #[test]
    fn t_zh_cn_menu() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("menu-file"), "文件");
        assert_eq!(i18n.t("menu-file-new"), "新建 (Ctrl+N)");
        assert_eq!(i18n.t("menu-edit"), "编辑");
    }

    #[test]
    fn t_en_us_menu() {
        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("menu-file"), "File");
        assert_eq!(i18n.t("menu-file-new"), "New (Ctrl+N)");
        assert_eq!(i18n.t("menu-edit"), "Edit");
    }

    #[test]
    fn t_actions_both_langs() {
        let mut i18n = I18n::new();

        i18n.set_lang(Lang::ZhCN);
        assert_eq!(i18n.t("action-save"), "保存");
        assert_eq!(i18n.t("action-undo"), "撤销");
        assert_eq!(i18n.t("view-source"), "源码");

        i18n.set_lang(Lang::EnUS);
        assert_eq!(i18n.t("action-save"), "Save");
        assert_eq!(i18n.t("action-undo"), "Undo");
        assert_eq!(i18n.t("view-source"), "Source");
    }

    #[test]
    fn tr_with_args() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        let mut args = FluentArgs::new();
        args.set("count", 5);
        // Fluent 会对数字应用 Unicode bidi 隔离，因此结果包含 U+2068/U+2069
        let result = i18n.tr("outline-heading", Some(&args));
        assert!(result.contains("大纲"));
        assert!(result.contains("5"));
    }

    #[test]
    fn tr_with_multiple_args() {
        let i18n = I18n::with_lang(Lang::EnUS);
        let mut args = FluentArgs::new();
        args.set("saved", 3);
        args.set("skipped", 1);
        let result = i18n.tr("status-save-skipped", Some(&args));
        assert!(result.contains("3"));
        assert!(result.contains("1"));
    }

    #[test]
    fn missing_key_returns_key_name() {
        let i18n = I18n::new();
        assert_eq!(i18n.t("nonexistent-key"), "nonexistent-key");
    }

    #[test]
    fn lang_set_switches_translations() {
        let mut i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("menu-file"), "文件");

        i18n.set_lang(Lang::EnUS);
        assert_eq!(i18n.t("menu-file"), "File");

        i18n.set_lang(Lang::ZhCN);
        assert_eq!(i18n.t("menu-file"), "文件");
    }

    #[test]
    fn lang_as_str_roundtrip() {
        assert_eq!("zh-CN".parse::<Lang>().unwrap(), Lang::ZhCN);
        assert_eq!("en-US".parse::<Lang>().unwrap(), Lang::EnUS);
        assert!("unknown".parse::<Lang>().is_err());
    }

    #[test]
    fn lang_display_name() {
        assert_eq!(Lang::ZhCN.display_name(), "中文");
        assert_eq!(Lang::EnUS.display_name(), "English");
    }

    #[test]
    fn settings_tab_translations() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("settings-tab-css"), "样式");
        assert_eq!(i18n.t("settings-tab-image"), "图片");
        assert_eq!(i18n.t("settings-tab-spell"), "拼写");
        assert_eq!(i18n.t("settings-tab-keybind"), "快捷键");

        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("settings-tab-css"), "Style");
        assert_eq!(i18n.t("settings-tab-image"), "Image");
    }

    #[test]
    fn confirm_dialog_translations() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(
            i18n.t("confirm-unsaved-body"),
            "当前文档有未保存修改。是否保存?"
        );
        assert_eq!(i18n.t("confirm-btn-save"), "保存");

        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(
            i18n.t("confirm-unsaved-body"),
            "The document has unsaved changes. Save?"
        );
        assert_eq!(i18n.t("confirm-btn-save"), "Save");
    }

    #[test]
    fn editor_ui_translations() {
        let i18n = I18n::with_lang(Lang::ZhCN);
        assert_eq!(i18n.t("search-find"), "查找:");
        assert_eq!(i18n.t("tab-close-others"), "关闭其他");
        assert_eq!(i18n.t("outline-empty"), "（无标题）");

        let i18n = I18n::with_lang(Lang::EnUS);
        assert_eq!(i18n.t("search-find"), "Find:");
        assert_eq!(i18n.t("tab-close-others"), "Close Others");
        assert_eq!(i18n.t("outline-empty"), "(No headings)");
    }
}
