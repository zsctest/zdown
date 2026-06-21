# 自定义快捷键映射 — 设计规格

**日期**: 2026-06-21  
**状态**: 已确认  
**阶段**: 3（剩余任务）

---

## 1. 概述

允许用户在设置对话框中查看和自定义键盘快捷键映射。采用 **delta 模式**：TOML 仅存储用户修改的绑定，未覆盖的使用代码默认值。

---

## 2. 数据模型

### 2.1 Action 枚举

`crates/config/src/keybinding.rs` — 所有可绑定快捷键的操作：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Save,
    SaveAs,
    NewFile,
    Open,
    CloseTab,
    NextTab,
    PrevTab,
    MoveTabLeft,
    MoveTabRight,
    Undo,
    Redo,
    ViewSource,
    ViewPreview,
    ViewHybrid,
    ToggleTheme,
}
```

`Action` 提供：
- `all()` → `&[Action; N]` — 遍历所有操作
- `default_binding()` → `KeyBinding` — 每个 action 的默认绑定
- `display_name()` → `&'static str` — 中文显示名

### 2.2 KeyBinding

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub modifiers: Modifiers,
    pub key: Key,  // egui::Key，序列化为字符串
}
```

`KeyBinding` 提供：
- `matches(input)` → `bool` — 判断当前输入是否匹配此绑定
- `display()` → `String` — 人类可读的快捷键字符串（如 "Ctrl+Shift+S"）
- `from_event(mods, key)` → `Option<Self>` — 从按键事件构建

### 2.3 Keymap (delta 模式)

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keymap {
    #[serde(default)]
    pub overrides: HashMap<Action, KeyBinding>,
}
```

`Keymap` 提供：
- `resolve(action)` → `KeyBinding` — 返回 override 或默认值
- `set_override(action, binding)` — 设置用户覆盖
- `clear_override(action)` — 恢复单个默认
- `clear_all()` — 恢复全部默认
- `detect_conflict(action, binding)` → `Option<Action>` — 冲突检测

### 2.4 AppConfig 集成

```rust
pub struct AppConfig {
    // ... 现有字段 ...
    #[serde(default)]
    pub keymap: Keymap,
}
```

TOML 序列化示例：
```toml
[keymap.overrides]
Save = { modifiers = { ctrl = true, shift = false, alt = false }, key = "S" }
ViewSource = { modifiers = { ctrl = false, shift = false, alt = true }, key = "D1" }
```

未在 overrides 中的 action 走默认值，不写入 TOML。

---

## 3. UI 设计

### 3.1 标签页布局

设置对话框新增第 4 个标签页「快捷键」，位于「样式 / 图片 / 拼写 / **快捷键**」。

**标签页内容：**
- 顶部提示文字 + 「恢复全部默认」按钮
- 可滚动表格，每行包含：
  - **操作名** — 中文显示名
  - **快捷键** — 当前绑定文本（如 `Ctrl+Shift+S`），可点击进入捕获模式
  - **恢复按钮** (↺) — 恢复该 action 到默认值

### 3.2 交互流程

**重新绑定：**
1. 用户点击快捷键单元格
2. 单元格进入「监听模式」，显示 `⏳ 按下新快捷键...`
3. 用户按下组合键（如 Ctrl+Shift+S）
4. 系统捕获按键和修饰键，执行冲突检测
5. 无冲突 → 更新 `keymap.overrides`，单元格刷新显示
6. 有冲突 → 标红显示冲突信息，仍允许保存
7. 用户按 Esc → 取消捕获，恢复原显示

**冲突检测规则：**
- 不同 action 绑定到相同按键组合 → 冲突
- 冲突在捕获时标红提示，但允许用户保存（后者覆盖前者）
- 仅检查与当前 `resolve()` 结果集（即生效绑定）是否冲突

**取消捕获的触发条件：**
- 按 Esc 键
- 焦点离开快捷键表格
- 点击其他 UI 元素

### 3.3 按键捕获实现

在 `settings_dialog.rs` 中增加状态字段：

```rust
pub struct KeybindingCapture {
    pub action: Action,
    pub conflict_with: Option<Action>,
}
```

渲染时：如果 `capture` 为 `Some`，当前帧消费掉按键事件并执行绑定更新。通过 `egui::Response::request_focus()` 确保焦点在当前单元格。

---

## 4. 运行时重构

### 4.1 handle_shortcuts 改为查表驱动

**现状**（menu.rs）— 每快捷键独立 if 判断：
```rust
if mods.ctrl && !mods.shift && ctx.input(|i| i.key_pressed(egui::Key::S)) { save(); }
// 重复 15 次
```

**目标** — 循环查表：
```rust
pub fn handle_shortcuts(
    ctx: &egui::Context,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    app_config: &AppConfig,
) {
    let mods = ctx.input(|i| i.modifiers);
    for action in Action::all() {
        let binding = app_config.keymap.resolve(*action);
        if binding.matches(mods, ctx) {
            execute_action(action, state, confirm, app_config);
        }
    }
}
```

### 4.2 execute_action 分发

将现有的 if 分支体提取为独立函数：
```rust
fn execute_action(
    action: &Action,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    app_config: &AppConfig,
) {
    match action {
        Action::Save => save(state, app_config),
        Action::SaveAs => save_as(state, app_config),
        Action::NewFile => state.new_file(),
        // ...
    }
}
```

### 4.3 新增 ToggleTheme

为 `Action::ToggleTheme` 新增 Ctrl+T 默认快捷键。此前主题切换仅通过菜单按钮触发，无快捷键。

---

## 5. 边界情况

| 场景 | 处理方式 |
|---|---|
| 用户仅按修饰键（如仅 Ctrl） | 忽略，不执行绑定 |
| 捕获模式下点击其他 UI | 自动取消捕获 |
| TOML 中有已删除的 action 名 | serde 反序列化时 warn! 日志 + 跳过 |
| 两个 action 绑定到相同按键 | 捕获时标红警告，允许保存 |
| 配置文件损坏 | serde 反序列化失败 → 使用 Default，日志记录 |
| 空 TOML / 首次运行 | keymap.overrides 为空，全部走默认 |

---

## 6. 文件变更

| 文件 | 变更 | 说明 |
|---|---|---|
| `crates/config/src/keybinding.rs` | 新增 | Action, KeyBinding, Keymap 定义 + 默认绑定 + 序列化 |
| `crates/config/src/lib.rs` | 修改 | AppConfig 增加 keymap 字段；重新导出 keybinding 模块 |
| `crates/zdown-app/src/settings_dialog.rs` | 修改 | 新增 Keybind 标签页 + 按键捕获逻辑 |
| `crates/zdown-app/src/menu.rs` | 修改 | handle_shortcuts 改为查表驱动；新增 execute_action 分发函数 |

---

## 7. 测试计划

### 单元测试 — keybinding.rs
- `Action::default_binding()` — 每个 action 返回非空默认绑定
- `Action::all()` — 覆盖所有 action 变体
- `Action::display_name()` — 返回非空中文字符串
- `KeyBinding::matches()` — 修饰键精确匹配，部分匹配返回 false
- `KeyBinding::display()` — 格式化输出（Ctrl+X, Ctrl+Shift+X, Alt+X 等）
- `KeyBinding` 序列化/反序列化往返
- `Keymap::resolve()` — 未覆盖返回默认
- `Keymap::resolve()` — 覆盖后返回自定义
- `Keymap::detect_conflict()` — 检测到冲突返回冲突 action
- `Keymap::detect_conflict()` — 无冲突返回 None
- `Keymap::detect_conflict()` — 同一 action 不视为冲突
- `Keymap` 序列化/反序列化往返
- 未知 action 名反序列化处理

### 单元测试 — settings_dialog.rs
- 打开对话框时 keymap 缓冲区正确填充
- 修改绑定后保存写入 app_config.keymap
- 恢复单个默认清空对应 override
- 恢复全部默认清空所有 overrides

### 单元测试 — lib.rs (config)
- `AppConfig` 带 keymap 的 TOML 序列化/反序列化往返
- 空 keymap → 默认值

---

## 8. 不包含的内容

- **文本编辑键配置**（Backspace, Delete, Enter, 方向键）— 保持硬编码
- **插件注册快捷键** — 插件系统实现后再扩展
- **快捷键导入/导出** — 后续版本
- **条件快捷键**（如"仅在编辑模式下生效"）— 当前所有 action 全局生效
