# 自定义编辑器字体设置 — 设计规格

**日期**: 2026-06-20
**状态**: 已批准
**阶段**: 3（个性化与多文件）

---

## 1. 概述

用户可以在设置对话框中选择编辑器等宽字体（影响源码/hybrid 编辑区），字号可配置，无需重启即可实时生效。设置持久化到 `config.toml`。

---

## 2. 配置结构

### 2.1 `EditorFontConfig`

```rust
/// 编辑器字体配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorFontConfig {
    /// 字体家族名称，如 "Cascadia Code"、"Fira Code"。
    /// 默认 "monospace" 表示使用系统默认等宽字体。
    pub family: String,
    /// 字号（pt），范围 8–32，默认 14.0。
    pub size: f32,
}

impl Default for EditorFontConfig {
    fn default() -> Self {
        Self {
            family: "monospace".to_string(),
            size: 14.0,
        }
    }
}
```

### 2.2 `AppConfig` 扩展

```rust
pub struct AppConfig {
    pub custom_css: Option<String>,
    pub theme: ThemeMode,
    pub image_hosting: ImageHostingConfig,
    #[serde(default)]
    pub editor_font: EditorFontConfig,  // 新增
}
```

- `#[serde(default)]` 保证旧配置文件自动填充默认值，向后兼容。
- `family = "monospace"` 时行为与当前无自定义字体一致。

---

## 3. 字体枚举

### 3.1 `font_provider` 模块

位置：`crates/zdown-app/src/font_provider.rs`

```rust
/// 字体提供者：枚举系统已安装的等宽字体。
pub struct FontProvider;

impl FontProvider {
    /// 返回系统所有等宽字体的家族名列表（去重排序）。
    pub fn list_monospace_families() -> Vec<String>;
    /// 按字体家族名查找 TTF 字节。使用 font-kit 系统查找。
    pub fn load_font_ttf(family: &str) -> Option<Vec<u8>>;
    /// 将等宽字体注册到 egui 上下文，替换 TextStyle::Monospace。
    pub fn register_editor_font(ctx: &egui::Context, family: &str, size: f32);
}
```

### 3.2 枚举逻辑

- 使用 `font_kit::source::SystemSource::new()` 遍历系统字体
- 过滤条件：`properties().style == Style::Normal` 且 `is_monospace() == true`
- 提取 `properties().postscript_name` 或 family name，去重按字母排序
- `"monospace"` 作为列表首项，显示为"系统默认等宽"
- 失败回退：返回 `vec!["monospace".to_string()]`
- 结果缓存在 `ZdownApp.available_fonts: Vec<String>`（启动时枚举一次）

---

## 4. 字体注册与实时生效

### 4.1 注册流程

```
用户选字体/字号 → register_editor_font(ctx, family, size)
                        │
                        ▼
            family != "monospace" ?
              │ yes           │ no
              ▼               ▼
    font-kit 查找 TTF     重置为 egui 内置等宽
              │
              ▼
    ctx.add_font(FontData::from_owned_ttf(bytes))
    ctx.set_fonts(覆盖 TextStyle::Monospace)
              │
              ▼
        编辑器立即用新字体渲染
```

### 4.2 降级链

| 优先级 | 条件 | 行为 |
|--------|------|------|
| 1 | `family = "monospace"` | 重置为 egui 内置默认等宽 |
| 2 | font-kit 找到 TTF | 加载注册，成功 |
| 3 | font-kit 找不到 | 保留当前字体不变，状态栏显示警告 |

### 4.3 生效时机

| 场景 | 行为 |
|------|------|
| 应用启动 | `ZdownApp::default()` 从 config 读取，非默认字体时注册 |
| 设置对话框打开 | 缓冲区填充当前配置 |
| 下拉选择/字号改变 | **即时**调用 `register_editor_font`（选字体即预览） |
| 点击"保存" | 写 `app_config.editor_font` + 调用 `app_config.save()` |
| 点击"取消" | 恢复到 `open_dialog` 时保存的字体 + 重注册 |

---

## 5. 设置对话框 UI

### 5.1 新增"字体"标签页

在"样式"和"图片"标签页之间插入：

```
[样式] [字体] [图片]
```

### 5.2 新增 SettingsTab 变体

```rust
#[derive(Debug, Clone, PartialEq)]
enum SettingsTab {
    Css,
    Font,   // 新增
    Image,
}
```

### 5.3 字体标签页布局

```
编辑器字体
[ComboBox: 包含 available_fonts 列表]

字号
[DragValue: 8–32] pt

预览
┌──────────────────────────────────┐
│ The quick brown fox jumps over   │
│ the lazy dog. 0123456789         │
│ 敏捷的棕狐狸跳过了那只懒狗。      │
│ fn main() { println!("hi"); }    │
└──────────────────────────────────┘
```

- **ComboBox**: egui 内置组件，数据源 `ZdownApp.available_fonts`，首项"系统默认等宽"
- **DragValue**: 范围 8–32，步长 1
- **预览区**: `egui::Frame` + `egui::Label`，用当前选择的字体渲染

### 5.4 `SettingsDialog` 扩展

```rust
pub struct SettingsDialog {
    // ... 现有字段 ...
    font_family_buffer: String,   // 选中的字体名
    font_size_buffer: f32,        // 选中的字号
}
```

### 5.5 `open_dialog` 签名变更

```rust
pub fn open_dialog(
    &mut self,
    current_css: Option<&str>,
    image_config: &ImageHostingConfig,
    editor_font: &EditorFontConfig,    // 新增
) { ... }
```

### 5.6 `show_settings_dialog` 签名变更

```rust
pub fn show_settings_dialog(
    ctx: &egui::Context,
    app_config: &mut AppConfig,
    dialog: &mut SettingsDialog,
    available_fonts: &[String],        // 新增
) { ... }
```

---

## 6. `ZdownApp` 字段变更

```rust
struct ZdownApp {
    // ... 现有字段 ...
    available_fonts: Vec<String>,  // 启动时枚举的系统等宽字体
}
```

Default 初始化时调用 `FontProvider::list_monospace_families()` 并注册 `editor_font`。

---

## 7. 数据流

```
启动
 └─> FontProvider::list_monospace_families() → available_fonts
 └─> AppConfig::load() → editor_font
      └─> register_editor_font(ctx, family, size)

打开设置
 └─> open_dialog(css, img, &editor_font)
      └─> font_family_buffer = family
      └─> font_size_buffer = size

用户选字体
 └─> ComboBox → font_family_buffer 更新
      └─> register_editor_font(ctx, new_family, size)  // 预览

用户调字号
 └─> DragValue → font_size_buffer 更新
      └─> register_editor_font(ctx, family, new_size)  // 预览

保存
 └─> app_config.editor_font = { family: buffer, size: buffer }
 └─> app_config.save() → config.toml

取消
 └─> register_editor_font(ctx, old_family, old_size)  // 恢复
```

---

## 8. 测试计划

### 单元测试（config crate）

- `editor_font_config_default` — 默认 family = "monospace", size = 14.0
- `editor_font_config_roundtrip` — 序列化→反序列化一致性
- `old_config_without_editor_font_defaults` — 旧 TOML 缺失字段自动默认

### 单元测试（zdown-app）

- `list_monospace_families_returns_non_empty` — 至少包含 "monospace"
- `load_font_ttf_monospace_returns_none` — "monospace" 不查找文件
- `load_font_ttf_invalid_name_returns_none` — 不存在字体返回 None

### 单元测试（settings_dialog）

- `open_dialog_populates_font_buffers` — 打开后缓冲区填充
- `font_tab_is_new_default` — 默认标签页不变

### 集成测试

- 手动验证：选字体 → 预览 → 保存 → 重启后字体保留
- 手动验证：字号变化编辑器即时更新
- 手动验证：取消操作恢复原字体

---

## 9. 依赖

- `font-kit` — 已是工作区间接依赖（通过 export_engine），复用

---

## 10. 不变更项

- 预览区/Markdown 渲染区字体不做变更（仅编辑器等宽字体）
- PDF 导出 `FontConfig` 不做变更（是独立配置）
- 不涉及 UI 主题字体（egui 界面自身）
