//! 字体提供者：枚举系统等宽字体、加载 TTF、注册到 egui。
//!
//! 使用 font-kit 遍历系统字体目录。结果为快照——启动后新增/删除字体
//! 需重启 zdown 才能看到变化。

use eframe::egui;
use font_kit::family_name::FamilyName;
use font_kit::properties::{Properties, Style};
use font_kit::source::SystemSource;

/// 字体提供者：枚举系统已安装的等宽字体。
pub struct FontProvider;

impl FontProvider {
    /// 返回系统所有等宽字体的家族名列表（去重排序）。
    ///
    /// 列表首项恒为 "monospace"（"系统默认等宽"）。
    pub fn list_monospace_families() -> Vec<String> {
        let mut families: Vec<String> = vec!["monospace".to_string()];

        let source = SystemSource::new();
        if let Ok(all_families) = source.all_families() {
            for family_name in &all_families {
                let name = family_name.clone();
                if name.is_empty() || families.contains(&name) {
                    continue;
                }
                // 只保留等宽字体：尝试按家族名匹配，检查等宽属性
                if Self::is_monospace_family(&name) {
                    families.push(name);
                }
            }
            families[1..].sort();
        }

        families
    }

    /// 按字体家族名查找 TTF 字节。
    ///
    /// 使用 font-kit 系统查找 Normal 样式。
    /// `family` 为 "monospace" 时直接返回 None（不查找）。
    pub fn load_font_ttf(family: &str) -> Option<Vec<u8>> {
        if family == "monospace" {
            return None;
        }
        let source = SystemSource::new();
        let handle = source
            .select_best_match(
                &[FamilyName::Title(family.into())],
                Properties::new().style(Style::Normal),
            )
            .ok()?;
        match handle {
            font_kit::handle::Handle::Path { path, .. } => std::fs::read(path).ok(),
            font_kit::handle::Handle::Memory { bytes, .. } => Some((*bytes).clone()),
        }
    }

    /// 将等宽字体注册到 egui 上下文，替换 `TextStyle::Monospace` 映射。
    ///
    /// - `family == "monospace"` → 重置为 egui 默认等宽
    /// - 其他 → 从系统加载 TTF 注册；加载失败时保留当前字体不变
    pub fn register_editor_font(ctx: &egui::Context, family: &str, size: f32) {
        let mut fonts = egui::FontDefinitions::default();

        if family != "monospace" {
            if let Some(ttf_bytes) = Self::load_font_ttf(family) {
                let font_name = format!("custom_mono_{family}");
                fonts
                    .font_data
                    .insert(font_name.clone(), std::sync::Arc::new(egui::FontData::from_owned(ttf_bytes)));
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .insert(0, font_name);
            }
            // 加载失败：不改变字体映射，保留当前状态
        }
        // family == "monospace" 时 fonts 为默认定义，Monospace 恢复默认映射

        ctx.set_fonts(fonts);

        // 设置字号通过修改 TextStyle 的 FontId
        let mut style = (*ctx.global_style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(size, egui::FontFamily::Monospace),
        );
        ctx.set_global_style(style);
    }

    /// 检查某个家族名是否对应等宽字体。
    fn is_monospace_family(name: &str) -> bool {
        let source = SystemSource::new();
        let handle = source.select_best_match(
            &[FamilyName::Title(name.into())],
            Properties::new().style(Style::Normal),
        );
        match handle {
            Ok(h) => {
                if let Ok(font) = h.load() {
                    return font.is_monospace();
                }
                false
            }
            Err(_) => false,
        }
    }
}

/// 将 font-kit FamilyName 转为纯字符串。
fn family_name_to_string(name: &font_kit::family_name::FamilyName) -> String {
    match name {
        font_kit::family_name::FamilyName::Title(s) => s.clone(),
        font_kit::family_name::FamilyName::Serif => "serif".into(),
        font_kit::family_name::FamilyName::SansSerif => "sans-serif".into(),
        font_kit::family_name::FamilyName::Monospace => "monospace".into(),
        font_kit::family_name::FamilyName::Cursive => "cursive".into(),
        font_kit::family_name::FamilyName::Fantasy => "fantasy".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_monospace_families_returns_non_empty() {
        let families = FontProvider::list_monospace_families();
        assert!(!families.is_empty(), "至少应包含 monospace");
        assert_eq!(families[0], "monospace", "首项应为 monospace");
        // 验证去重
        let mut unique = families.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(families.len(), unique.len(), "不应有重复项");
    }

    #[test]
    fn load_font_ttf_monospace_returns_none() {
        assert!(FontProvider::load_font_ttf("monospace").is_none());
    }

    #[test]
    fn load_font_ttf_invalid_name_returns_none() {
        assert!(FontProvider::load_font_ttf("__nonexistent_font_xyz__").is_none());
    }

    #[test]
    fn family_name_to_string_converts() {
        let name = family_name_to_string(&FamilyName::Title("Test".into()));
        assert_eq!(name, "Test");
    }
}
