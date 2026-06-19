# 主题切换功能 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 zdown 添加亮色/暗色主题切换，视图菜单 + AppConfig 持久化，代码高亮自动跟随。

**架构：** `ThemeMode` 枚举（config crate）→ `AppConfig.theme` 持久化 → `ZdownApp` 持有当前主题 → 启动时应用 `egui::Visuals` + `SourceHighlighter` 主题 → 菜单切换触发即时应用 + 保存 + highlighter 重建。

**技术栈：** Rust 2024 Edition, egui Visuals, syntect ThemeSet, serde/TOML

---

### 任务 1：ThemeMode 枚举 + AppConfig.theme + 测试

**文件：**
- 修改：`crates/config/src/lib.rs`

- [ ] **步骤 1：添加 ThemeMode 枚举和 AppConfig.theme 字段**

在 `AppConfig` 结构体定义之前（第 21 行之前）插入 `ThemeMode` 枚举：

```rust
/// 主题模式。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}
```

在 `AppConfig` 结构体的 `custom_css` 字段之后（第 30 行之后）添加：

```rust
    /// UI 主题：暗色或亮色。默认暗色。
    pub theme: ThemeMode,
```

- [ ] **步骤 2：添加 config 测试**

在 `#[cfg(test)] mod tests` 中（第 165 行之前）添加：

```rust
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
        };
        let toml_str = toml::to_string_pretty(&config).expect("serialize");
        assert!(toml_str.contains("theme"), "TOML 应包含 theme 字段: {toml_str}");
        assert!(toml_str.contains("Light"), "TOML 应包含 Light: {toml_str}");
    }
```

- [ ] **步骤 3：运行 config 测试**

```powershell
cargo test -p config
```
预期：原有 8 个测试 + 新增 4 个测试 = 12 个全部 PASS

- [ ] **步骤 4：Commit**

```bash
git add crates/config/src/lib.rs
git commit -m "feat(config): add ThemeMode enum and AppConfig.theme field

- ThemeMode: Dark (default) / Light
- AppConfig.theme with serde(default) for backward compatibility
- 4 new tests: default, roundtrip, old config compat, TOML serialization

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 2：ZdownApp 集成 — 应用主题 + highlighter 重建

**文件：**
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：添加 theme 字段到 ZdownApp + 导入**

在文件顶部的导入区，添加：

```rust
use config::ThemeMode;
```

在 `ZdownApp` 结构体的 `search: SearchState` 行后添加：

```rust
    /// 当前主题模式。
    theme: ThemeMode,
```

修改 `Default` 实现：先从 `AppConfig::load()` 获取配置并提取 `theme`，再构造 `Self`。

将当前的 `Default` 实现（约第 76-93 行）改为：

```rust
impl Default for ZdownApp {
    fn default() -> Self {
        let app_config = config::AppConfig::load().unwrap_or_default();
        let theme = app_config.theme.clone();
        Self {
            state: EditorState::default(),
            confirm: ConfirmDialog::default(),
            view_mode: ViewMode::default(),
            last_title: String::new(),
            highlighter: markdown_renderer::SourceHighlighter::new().ok(),
            render_cache: markdown_renderer::RenderCache::new(),
            fold_state: outline_view::OutlineFoldState::default(),
            app_config,
            settings_dialog: settings_dialog::SettingsDialog::default(),
            search: SearchState::default(),
            theme,
        }
    }
}
```

关键变更：`app_config` 的初始化从字段内联改为 `let` 绑定，同时提取 `theme`。

- [ ] **步骤 2：在 ui() 开始时应用主题**

在 `ui()` 方法开始处（第 90 行，`let ctx = ui.ctx().clone();` 之后），添加主题应用：

```rust
        let ctx = ui.ctx().clone();

        // 应用当前主题到 egui
        match self.theme {
            ThemeMode::Dark => ctx.set_visuals(egui::Visuals::dark()),
            ThemeMode::Light => ctx.set_visuals(egui::Visuals::light()),
        }
```

- [ ] **步骤 3：传递 theme 到 show_menu，接收变化并处理**

将 `show_menu` 调用（第 91-98 行）修改，添加 `theme` 参数传递。

将：
```rust
        let theme_before = self.theme.clone();
        menu::show_menu(
            ui,
            &mut self.state,
            &mut self.confirm,
            &mut self.view_mode,
            &mut self.settings_dialog,
            &self.app_config,
        );
```

改为：
```rust
        let theme_before = self.theme.clone();
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

在 `show_menu` 调用之后，检测主题变化并应用：

```rust
        // 主题切换时重建 highlighter + 保存配置
        if self.theme != theme_before {
            let syntax_name = match self.theme {
                ThemeMode::Dark => "base16-ocean.dark",
                ThemeMode::Light => "InspiredGitHub",
            };
            self.highlighter = markdown_renderer::SourceHighlighter::with_theme(syntax_name)
                .or_else(|_| {
                    tracing::warn!("语法主题加载失败: {syntax_name}，使用默认");
                    markdown_renderer::SourceHighlighter::new()
                })
                .ok();

            self.app_config.theme = self.theme.clone();
            if let Err(e) = self.app_config.save() {
                tracing::error!("配置保存失败: {e}");
            }
        }
```

- [ ] **步骤 4：编译验证**

```powershell
cargo check -p zdown-app
```
预期：编译失败 — `show_menu` 签名尚未更新（任务 3 处理）

- [ ] **步骤 5：Commit**

```bash
git add crates/zdown-app/src/main.rs
git commit -m "feat(theme): apply theme on startup and rebuild highlighter on change

- Add theme field to ZdownApp, load from AppConfig on init
- Apply egui Visuals on each frame
- Detect theme change after show_menu, recreate SourceHighlighter

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 3：视图菜单添加主题切换项

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`

- [ ] **步骤 1：更新 show_menu 签名，添加 theme 参数**

将第 34-41 行的函数签名从：

```rust
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    settings_dialog: &mut SettingsDialog,
    app_config: &AppConfig,
) {
```

改为：

```rust
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    settings_dialog: &mut SettingsDialog,
    app_config: &AppConfig,
    theme: &mut ThemeMode,
) {
```

在文件顶部导入区添加：

```rust
use config::ThemeMode;
```

- [ ] **步骤 2：在视图菜单中添加主题切换项**

找到视图菜单部分（第 121-131 行），在 `Hybrid (Ctrl+3)` 按钮之后、关闭 `ui.menu_button` 之前，添加分隔线和主题切换项。

将第 121-131 行的视图菜单：

```rust
            ui.menu_button("视图", |ui| {
                if ui.button("源码 (Ctrl+1)").clicked() {
                    *view_mode = ViewMode::Source;
                }
                if ui.button("预览 (Ctrl+2)").clicked() {
                    *view_mode = ViewMode::Preview;
                }
                if ui.button("Hybrid (Ctrl+3)").clicked() {
                    *view_mode = ViewMode::Hybrid;
                }
            });
```

改为：

```rust
            ui.menu_button("视图", |ui| {
                if ui.button("源码 (Ctrl+1)").clicked() {
                    *view_mode = ViewMode::Source;
                }
                if ui.button("预览 (Ctrl+2)").clicked() {
                    *view_mode = ViewMode::Preview;
                }
                if ui.button("Hybrid (Ctrl+3)").clicked() {
                    *view_mode = ViewMode::Hybrid;
                }

                ui.separator();

                // 主题切换：显示可切换到的目标主题
                let toggle_label = match theme {
                    ThemeMode::Dark => "☀️ 亮色主题",
                    ThemeMode::Light => "🌙 暗色主题",
                };
                if ui.button(toggle_label).clicked() {
                    *theme = match theme {
                        ThemeMode::Dark => ThemeMode::Light,
                        ThemeMode::Light => ThemeMode::Dark,
                    };
                    ui.close();
                }
            });
```

- [ ] **步骤 3：编译验证**

```powershell
cargo check -p zdown-app
```
预期：编译成功，无错误

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/menu.rs
git commit -m "feat(theme): add theme toggle to view menu

- Theme toggle item at bottom of view menu
- Shows target theme (not current): 亮色/暗色
- passes &mut ThemeMode to allow main.rs to detect changes

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 4：完整验证

- [ ] **步骤 1：运行所有测试**

```powershell
cargo test
```
预期：全部通过

- [ ] **步骤 2：clippy**

```powershell
cargo clippy -- -D warnings
```
预期：无警告

- [ ] **步骤 3：fmt**

```powershell
cargo fmt -- --check
cargo fmt  # 如有改动
```

- [ ] **步骤 4：最终提交**

```bash
git add -u && git commit -m "style: cargo fmt"  # 如有格式修正
```
