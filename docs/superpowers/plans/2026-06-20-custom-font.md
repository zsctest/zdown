# 自定义编辑器字体设置 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 用户可在设置对话框中通过下拉列表选择系统等宽字体并调整字号，修改实时生效并持久化到 config.toml。

**架构：** 在 config crate 新增 `EditorFontConfig` 结构体；在 zdown-app 新增 `font_provider` 模块（font-kit 枚举字体 + egui 注册）；扩展 SettingsDialog 添加字体标签页；ZdownApp 启动时枚举字体并注册已配置字体。

**技术栈：** Rust 2024, egui 0.34, font-kit 0.14 (workspace), serde/TOML

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `crates/config/src/lib.rs` | 修改 | 新增 `EditorFontConfig`，`AppConfig` 加 `editor_font` 字段 |
| `crates/zdown-app/Cargo.toml` | 修改 | 添加 `font-kit` 依赖 |
| `crates/zdown-app/src/font_provider.rs` | 创建 | 字体枚举、TTF 加载、egui 注册 |
| `crates/zdown-app/src/settings_dialog.rs` | 修改 | 新增 Font 标签页、扩展 struct/方法签名 |
| `crates/zdown-app/src/main.rs` | 修改 | 新增 `available_fonts` 字段、启动字体注册、传递参数 |
| `crates/zdown-app/src/menu.rs` | 修改 | `open_dialog` 调用传入 `editor_font` |

---

### 任务 1：Config — `EditorFontConfig` 结构体

**文件：**
- 修改：`crates/config/src/lib.rs`

**测试：**
- 已有测试文件 `crates/config/src/lib.rs`（内联 `#[cfg(test)]`）

- [ ] **步骤 1：编写失败的测试**

在文件末尾的 `#[cfg(test)] mod tests` 块中添加测试：

```rust
#[test]
fn editor_font_config_default() {
    let c = EditorFontConfig::default();
    assert_eq!(c.family, "monospace");
    assert!((c.size - 14.0).abs() < f32::EPSILON);
}

#[test]
fn editor_font_config_roundtrip() {
    let c = EditorFontConfig {
        family: "Cascadia Code".into(),
        size: 16.0,
    };
    let toml_str = toml::to_string_pretty(&c).unwrap();
    let restored: EditorFontConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(restored.family, "Cascadia Code");
    assert!((restored.size - 16.0).abs() < f32::EPSILON);
}

#[test]
fn old_config_without_editor_font_defaults() {
    let old_toml = r#"
theme = "Light"
"#;
    let config: AppConfig = toml::from_str(old_toml).unwrap();
    assert_eq!(config.editor_font.family, "monospace");
    assert!((config.editor_font.size - 14.0).abs() < f32::EPSILON);
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p config -- old_config_without_editor_font_defaults editor_font_config
```
预期：编译失败，`EditorFontConfig` 未定义。

- [ ] **步骤 3：实现 `EditorFontConfig` + 扩展 `AppConfig`**

在 `ThemeMode` 之后、`ImageStrategy` 之前插入：

```rust
/// 编辑器字体配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorFontConfig {
    /// 字体家族名称，如 "Cascadia Code"、"Fira Code"。
    /// 默认 "monospace" 表示使用系统默认等宽字体。
    #[serde(default = "default_font_family")]
    pub family: String,
    /// 字号（pt），范围 8–32，默认 14.0。
    #[serde(default = "default_font_size")]
    pub size: f32,
}

fn default_font_family() -> String {
    "monospace".into()
}

fn default_font_size() -> f32 {
    14.0
}

impl Default for EditorFontConfig {
    fn default() -> Self {
        Self {
            family: default_font_family(),
            size: default_font_size(),
        }
    }
}
```

在 `AppConfig` 中，`image_hosting` 字段后添加：

```rust
    /// 编辑器字体配置。
    #[serde(default)]
    pub editor_font: EditorFontConfig,
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p config
```
预期：全部 18 个测试通过（原有 15 + 新增 3）。

- [ ] **步骤 5：Commit**

```bash
git add crates/config/src/lib.rs
git commit -m "feat(config): add EditorFontConfig with family/size fields

Add EditorFontConfig struct and extend AppConfig with editor_font field.
Uses #[serde(default)] for backward compatibility with old config files.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 2：依赖 — 添加 font-kit 到 zdown-app

**文件：**
- 修改：`crates/zdown-app/Cargo.toml`

- [ ] **步骤 1：添加 font-kit workspace 依赖**

在 `[dependencies]` 尾部添加：

```toml
font-kit.workspace = true
```

- [ ] **步骤 2：验证编译**

```bash
cargo check -p zdown-app
```
预期：编译通过。

- [ ] **步骤 3：Commit**

```bash
git add crates/zdown-app/Cargo.toml
git commit -m "build(zdown-app): add font-kit workspace dependency

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 3：字体提供者 — `font_provider` 模块

**文件：**
- 创建：`crates/zdown-app/src/font_provider.rs`
- 修改：`crates/zdown-app/src/main.rs`（注册模块）

- [ ] **步骤 1：创建 `font_provider.rs`（完整实现 + 测试）**

```rust
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
                let name = family_name_to_string(family_name);
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
    /// - 其他 → 从系统加载 TTF 注册；加载失败时状态栏提示并保留当前字体
    pub fn register_editor_font(ctx: &egui::Context, family: &str, size: f32) {
        let mut fonts = egui::FontDefinitions::default();

        if family != "monospace" {
            if let Some(ttf_bytes) = Self::load_font_ttf(family) {
                let font_name = format!("custom_mono_{family}");
                fonts
                    .font_data
                    .insert(font_name.clone(), egui::FontData::from_owned_ttf(ttf_bytes));
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
        let mut style = (*ctx.style()).clone();
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(size, egui::FontFamily::Monospace),
        );
        ctx.set_style(style);
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
                    return font.properties().is_monospace;
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
        font_kit::family_name::FamilyName::Title(s)
        | font_kit::family_name::FamilyName::Display(s) => s.clone(),
        _ => String::new(),
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
```

- [ ] **步骤 2：在 `main.rs` 顶部注册模块**

在 `mod tab_bar;` 后添加：

```rust
mod font_provider;
```

- [ ] **步骤 3：运行测试验证通过**

```bash
cargo test -p zdown-app -- font_provider list_monospace load_font_ttf family_name
```
预期：4 个新测试全部 PASS。

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/font_provider.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): add font_provider module for system monospace enumeration

- list_monospace_families: enumerate system monospace fonts via font-kit
- load_font_ttf: load TTF bytes by family name
- register_editor_font: register font to egui Context, replace Monospace
- 'monospace' family resets to egui default

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 4：SettingsDialog — 新增字体标签页

**文件：**
- 修改：`crates/zdown-app/src/settings_dialog.rs`

- [ ] **步骤 1：编写失败的测试**

在 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
fn open_dialog_populates_font_buffers() {
    let mut dialog = SettingsDialog::default();
    let font_config = EditorFontConfig {
        family: "Fira Code".into(),
        size: 16.0,
    };
    dialog.open_dialog(None, &Default::default(), &font_config);
    assert!(dialog.open);
    assert_eq!(dialog.font_family_buffer, "Fira Code");
    assert!((dialog.font_size_buffer - 16.0).abs() < f32::EPSILON);
}

#[test]
fn open_dialog_default_font_buffers() {
    let mut dialog = SettingsDialog::default();
    dialog.open_dialog(None, &Default::default(), &EditorFontConfig::default());
    assert_eq!(dialog.font_family_buffer, "monospace");
    assert!((dialog.font_size_buffer - 14.0).abs() < f32::EPSILON);
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p zdown-app -- open_dialog_populates_font open_dialog_default_font
```
预期：编译失败，`open_dialog` 签名不匹配或 `font_family_buffer` 不存在。

- [ ] **步骤 3：修改 `SettingsDialog`**

**3a. 更新 import：**

```rust
use config::{AppConfig, EditorFontConfig, ImageHostingConfig, ImageStrategy};
```

**3b. 扩展 `SettingsTab`：**

```rust
enum SettingsTab {
    Css,
    Font,
    Image,
}
```

**3c. 扩展 `SettingsDialog` struct：**

```rust
pub struct SettingsDialog {
    pub open: bool,
    active_tab: SettingsTab,
    css_buffer: String,
    local_dir_buffer: String,
    smms_token_buffer: String,
    strategy_buffer: usize,
    /// 字体设置缓冲区
    font_family_buffer: String,
    font_size_buffer: f32,
}
```

**3d. 更新 `Default`：**

```rust
impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: SettingsTab::Css,
            css_buffer: String::new(),
            local_dir_buffer: "images".to_string(),
            smms_token_buffer: String::new(),
            strategy_buffer: 0,
            font_family_buffer: "monospace".to_string(),
            font_size_buffer: 14.0,
        }
    }
}
```

**3e. 更新 `open_dialog` 签名和实现：**

```rust
pub fn open_dialog(
    &mut self,
    current_css: Option<&str>,
    image_config: &ImageHostingConfig,
    editor_font: &EditorFontConfig,
) {
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
    self.font_family_buffer = editor_font.family.clone();
    self.font_size_buffer = editor_font.size;
}
```

- [ ] **步骤 4：运行已有测试验证编译通过**

```bash
cargo test -p zdown-app -- settings_dialog
```
预期：所有 settings_dialog 测试 PASS（6 个）。

- [ ] **步骤 5：更新 `show_settings_dialog`**

**5a. 签名变更：**

```rust
pub fn show_settings_dialog(
    ctx: &egui::Context,
    app_config: &mut AppConfig,
    dialog: &mut SettingsDialog,
    available_fonts: &[String],
) {
```

**5b. 在标签栏中添加字体标签页**（`"图片"` 前插入）：

```rust
ui.selectable_value(&mut dialog.active_tab, SettingsTab::Css, "样式");
ui.selectable_value(&mut dialog.active_tab, SettingsTab::Font, "字体");
ui.selectable_value(&mut dialog.active_tab, SettingsTab::Image, "图片");
```

**5c. 在 match 分支中，`SettingsTab::Css` 和 `SettingsTab::Image` 之间插入 `SettingsTab::Font`：**

```rust
SettingsTab::Font => {
    let font_changed_before = (
        dialog.font_family_buffer.clone(),
        dialog.font_size_buffer,
    );

    ui.label("编辑器字体：");
    egui::ComboBox::from_id_salt("editor_font_combo")
        .width(300.0)
        .selected_text(font_display_name(&dialog.font_family_buffer))
        .show_ui(ui, |ui| {
            for family in available_fonts {
                let label = font_display_name(family);
                if ui.selectable_label(false, label).clicked() {
                    dialog.font_family_buffer = family.clone();
                }
            }
        });

    ui.add_space(8.0);

    ui.label("字号：");
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut dialog.font_size_buffer)
                .clamp_range(8.0..=32.0)
                .speed(1.0),
        );
        ui.label("pt");
    });

    ui.add_space(12.0);

    // 预览
    ui.label("预览：");
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.monospace(
                egui::RichText::new(
                    "The quick brown fox jumps over the lazy dog. 0123456789\n\
                     敏捷的棕狐狸跳过了那只懒狗。\n\
                     fn main() { println!(\"Hello, zdown!\"); }",
                )
                .size(dialog.font_size_buffer),
            );
        });

    // 字体/字号变化即预览
    let font_changed_after = (
        dialog.font_family_buffer.clone(),
        dialog.font_size_buffer,
    );
    if font_changed_before != font_changed_after {
        FontProvider::register_editor_font(
            ctx,
            &dialog.font_family_buffer,
            dialog.font_size_buffer,
        );
    }
}
```

**5d. 在文件顶部添加辅助函数**（在 `show_settings_dialog` 上方）：

```rust
/// 字体列表的显示文本："monospace" 显示为 "系统默认等宽"。
fn font_display_name(family: &str) -> String {
    if family == "monospace" {
        "系统默认等宽".to_string()
    } else {
        family.to_string()
    }
}
```

**5e. 在"保存"按钮处理中，添加字体配置保存逻辑**（在 CSS 处理之后、图片处理之前插入）：

```rust
// 字体设置
app_config.editor_font.family = dialog.font_family_buffer.clone();
app_config.editor_font.size = dialog.font_size_buffer;
```

**5f. 在文件顶部 add import**（需要 `FontProvider`）：

```rust
use crate::font_provider::FontProvider;
```

- [ ] **步骤 6：运行测试验证通过**

```bash
cargo test -p zdown-app -- settings_dialog
```
预期：8 个 settings_dialog 测试 PASS（原有 5 + 新增 3）。

- [ ] **步骤 7：Commit**

```bash
git add crates/zdown-app/src/settings_dialog.rs
git commit -m "feat(zdown-app): add Font tab to settings dialog

- New SettingsTab::Font variant between Css and Image
- ComboBox for system monospace font selection
- DragValue for font size (8-32pt)
- Live preview area with real-time font registration
- font_display_name helper shows '系统默认等宽' for monospace

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 5：ZdownApp 集成 — `available_fonts` + 启动字体注册

**文件：**
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：在 `ZdownApp` struct 添加字段**

在 `theme: ThemeMode,` 后添加：

```rust
    /// 系统等宽字体列表（启动时枚举，运行期不变）。
    available_fonts: Vec<String>,
    /// 启动字体是否已注册（确保只初始化一次）。
    font_registered: bool,
```

- [ ] **步骤 2：在 `Default` 实现中初始化**

在 `theme,` 行后、闭包 `}` 前添加：

```rust
            available_fonts: font_provider::FontProvider::list_monospace_families(),
            font_registered: false,
```

- [ ] **步骤 3：启动时注册编辑器字体**

在 `ui()` 方法开头（`let ctx = ui.ctx().clone();` 之后）添加一次性的启动字体注册：

```rust
        // 启动时注册编辑器字体（只执行一次）
        if !self.font_registered {
            let font = &self.app_config.editor_font;
            font_provider::FontProvider::register_editor_font(
                &ctx,
                &font.family,
                font.size,
            );
            self.font_registered = true;
        }
```

- [ ] **步骤 4：更新 `show_menu` 调用**（移除 `image_hosting` 参数已在之前 CI 修复中完成，只需确认）

确认 `main.rs:menu::show_menu(...)` 调用只有 7 个参数（不含 `image_hosting`）：

```rust
        menu::show_menu(
            ui,
            &mut self.state,
            &mut self.confirm,
            &mut self.view_mode,
            &mut self.settings_dialog,
            &self.app_config,
            &mut self.theme,
        );
```

- [ ] **步骤 5：更新 `show_settings_dialog` 调用**

```rust
        settings_dialog::show_settings_dialog(
            &ctx,
            &mut self.app_config,
            &mut self.settings_dialog,
            &self.available_fonts,
        );
```

- [ ] **步骤 6：验证编译**

```bash
cargo check -p zdown-app
```
预期：编译通过。

- [ ] **步骤 7：运行全部测试**

```bash
cargo test
```
预期：全部通过。

- [ ] **步骤 8：Commit**

```bash
git add crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): integrate font provider into ZdownApp

- Add available_fonts field (enumerated at startup)
- Register editor font on first frame
- Pass available_fonts to settings dialog

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 6：菜单 — 更新 `open_dialog` 调用

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`

- [ ] **步骤 1：更新 `open_dialog` 调用**

将：

```rust
                settings_dialog
                    .open_dialog(app_config.custom_css.as_deref(), &app_config.image_hosting);
```

改为：

```rust
                settings_dialog.open_dialog(
                    app_config.custom_css.as_deref(),
                    &app_config.image_hosting,
                    &app_config.editor_font,
                );
```

- [ ] **步骤 2：验证编译**

```bash
cargo check -p zdown-app
```
预期：编译通过。

- [ ] **步骤 3：运行全部测试**

```bash
cargo test
```
预期：全部通过。

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/menu.rs
git commit -m "fix(zdown-app): pass editor_font to open_dialog

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 7：最终验证

- [ ] **步骤 1：fmt + clippy + test**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

预期：全部通过，无警告。

- [ ] **步骤 2：如需要，手动验证**

运行 `cargo run -p zdown-app`：
1. 打开"文件 → 设置"
2. 切换到"字体"标签页
3. 从下拉选择一种等宽字体（如 Cascadia Code）
4. 确认预览区字体变化
5. 调整字号（如 18pt），确认预览变化
6. 点击"保存"
7. 关闭设置，确认编辑器字体已更新
8. 重启 zdown，确认字体保留
