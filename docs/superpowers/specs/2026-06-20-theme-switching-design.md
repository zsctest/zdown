# 主题切换功能 — 设计规格

**日期**：2026-06-20
**状态**：已批准

---

## 1. 功能概述

为 zdown 添加亮色/暗色主题切换。用户通过"视图"菜单切换主题，选择持久化到 AppConfig。egui 原生支持 Visuals::light()/dark()，代码高亮主题自动跟随。

### 1.1 核心需求

- 亮色/暗色二选一，默认暗色（保持当前行为）
- "视图"菜单中添加主题切换项
- 主题选择持久化到 AppConfig（TOML）
- 代码语法高亮主题自动跟随
- 即时生效，无需重启

### 1.2 非需求

- 多主题预设（Sepia、High Contrast 等）
- 语法高亮主题独立选择
- 跟随系统主题自动切换（auto mode）
- 自定义主题颜色配置

---

## 2. 架构设计

### 2.1 数据模型

**`config/src/lib.rs`**：

```rust
/// 主题模式。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub custom_css: Option<String>,
    pub theme: ThemeMode,  // 新增
}
```

`#[serde(default)]` 确保旧配置文件（无 `theme` 字段）自动使用 Dark 默认值。

### 2.2 主题应用逻辑

**`zdown-app/src/main.rs`**：

```rust
/// 应用主题到 egui context。
fn apply_theme(ctx: &egui::Context, mode: &ThemeMode) {
    match mode {
        ThemeMode::Dark => ctx.set_visuals(egui::Visuals::dark()),
        ThemeMode::Light => ctx.set_visuals(egui::Visuals::light()),
    }
}

/// 根据主题模式返回 syntect 主题名。
fn syntax_theme(mode: &ThemeMode) -> &str {
    match mode {
        ThemeMode::Dark => "base16-ocean.dark",
        ThemeMode::Light => "InspiredGitHub",
    }
}
```

### 2.3 修改点

| 文件 | 改动 |
|------|------|
| `crates/config/src/lib.rs` | 新增 `ThemeMode` 枚举；`AppConfig` 新增 `theme: ThemeMode` 字段 |
| `crates/zdown-app/src/main.rs` | `ZdownApp` 新增 `theme: ThemeMode` 字段；启动应用主题；切换时重建 highlighter |
| `crates/zdown-app/src/menu.rs` | `show_menu` 视图菜单中新增主题切换项 |

---

## 3. UI 设计

### 3.1 菜单项

视图菜单底部，分隔线后：

```
视图 ▼
  ├── 源码 (Ctrl+1)
  ├── 预览 (Ctrl+2)
  ├── Hybrid (Ctrl+3)
  ├── ──────────
  └── ☀️ 亮色主题     ← 当前暗色时显示，点击切亮色
      🌙 暗色主题     ← 当前亮色时显示，点击切暗色
```

菜单项文本显示**可切换到的**主题名（非当前主题名）。

---

## 4. 数据流

### 4.1 启动流程

```
AppConfig::load()
  → theme: Dark (默认，或上次保存的值)
  → apply_theme(ctx, dark)
  → SourceHighlighter::with_theme("base16-ocean.dark")
  → 应用启动
```

### 4.2 切换流程

```
用户点击 "☀️ 亮色主题"
  → self.theme = ThemeMode::Light
  → apply_theme(ctx, &Light)
  → highlighter = SourceHighlighter::with_theme("InspiredGitHub")
  → app_config.theme = Light
  → app_config.save()
```

---

## 5. 边界情况

| 场景 | 处理 |
|------|------|
| 旧配置文件无 theme 字段 | `#[serde(default)]` → `ThemeMode::Dark` |
| 切换主题时编辑器内容 | 不受影响，仅 UI 外观变化 |
| `SourceHighlighter::with_theme` 失败 | 回退到 `SourceHighlighter::new()`（默认 dark 主题） |
| 配置文件保存失败 | 仅 log error，不阻塞 UI（与现有 `custom_css` 保存逻辑一致） |
| 快速连续切换 | egui `set_visuals` 即时生效，无累积问题 |

---

## 6. 测试策略

### 6.1 单元测试（config）

- `ThemeMode::default()` 为 Dark
- `ThemeMode` 序列化/反序列化
- 旧格式 TOML（无 theme 字段）反序列化 → theme 为 Dark
- 新格式 TOML 完整 roundtrip

### 6.2 集成测试

- 启动时应用保存的主题
- 切换主题后菜单项文本变化
- 切换后配置持久化

---

## 7. 文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/config/src/lib.rs` | 修改 | 新增 ThemeMode 枚举 + AppConfig.theme 字段 |
| `crates/zdown-app/src/main.rs` | 修改 | ZdownApp 新增 theme 字段 + apply_theme + highlighter 重建 |
| `crates/zdown-app/src/menu.rs` | 修改 | 视图菜单新增主题切换项 |
