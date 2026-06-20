# 拼写检查 — 设计规格

**日期**：2026-06-21
**状态**：已批准

---

## 1. 功能概述

为 zdown 编辑器源码视图添加英文拼写检查。保存时自动检查，错误单词以红色波浪下划线标记。

### 1.1 核心需求

- 仅英文拼写检查（内置 en_US 词典）
- 仅源码视图生效
- 保存时触发检查（Ctrl+S）
- 错误单词显示红色波浪下划线
- 设置对话框新增"拼写"标签页，含启用/禁用开关
- AppConfig TOML 持久化开关状态

### 1.2 非需求（本次不实现）

- 右键替换建议菜单
- 多语言支持
- 混合视图/预览视图中的拼写标记
- 实时输入检查
- 用户自定义词典

---

## 2. 架构设计

### 2.1 新增 Crate：`crates/spellcheck/`

```
crates/spellcheck/
  ├── Cargo.toml          ← nuspell = "0.6"
  ├── src/
  │   ├── lib.rs          ← SpellChecker 公开接口
  │   ├── checker.rs      ← nuspell 封装
  │   └── dict/
  │       ├── en_US.aff   ← Hunspell 词缀规则
  │       └── en_US.dic   ← 英文词表
  └── build.rs            ← 不需要（include_bytes! 静态嵌入）
```

**Cargo.toml**：
```toml
[package]
name = "spellcheck"
version = "0.1.0"
edition = "2024"

[dependencies]
nuspell = "0.6"
```

### 2.2 核心接口

```rust
// crates/spellcheck/src/lib.rs

/// 拼写错误信息
#[derive(Debug, Clone, PartialEq)]
pub struct SpellError {
    /// 错误单词
    pub word: String,
    /// 在原文中的字节偏移 (start, end)
    pub span: (usize, usize),
}

/// 拼写检查器（线程安全、一次性构建）
pub struct SpellChecker {
    dict: nuspell::Dictionary,
}

impl SpellChecker {
    /// 从嵌入的 en_US 词典构建。
    /// 词典文件通过 include_bytes! 在编译时嵌入。
    pub fn new() -> Result<Self, SpellcheckError>;

    /// 检查整段文本，返回所有拼写错误。
    /// 跳过：纯数字、URL、代码块内容（以反引号包裹的）、
    /// 以及长度 ≤ 1 的 token。
    pub fn check(&self, text: &str) -> Vec<SpellError>;

    /// 检查单个单词是否拼写正确。
    pub fn check_word(&self, word: &str) -> bool;
}
```

### 2.3 check() 算法

```
输入：Markdown 源码全文
  1. 逐行遍历
  2. 跳过代码围栏块（``` ... ```）
  3. 跳过行内代码（`...`）
  4. 对每行非代码文本，按非字母字符 split 提取单词 token
  5. 跳过长度 ≤ 1 的 token
  6. 跳过纯数字 token
  7. 跳过 URL（含 :// 的 token）
  8. 对每个有效 token 调用 nuspell::Dictionary::check()
  9. 若返回 false，记录 SpellError { word, span: (start_byte, end_byte) }
```

### 2.4 集成点

| 文件 | 改动 |
|------|------|
| `crates/spellcheck/**` | 新增 crate |
| `crates/zdown-app/Cargo.toml` | 添加 `spellcheck` 依赖 |
| `crates/zdown-app/src/editor_state.rs` | `EditorState` 新增 `spell_checker: SpellChecker` + `spell_errors: Vec<SpellError>`；`save()`/`save_as()` 成功保存后触发检查 |
| `crates/zdown-app/src/source_view.rs` | 渲染波浪下划线（读取 `state.spell_errors`，在 `render_text_with_cursor` 中追加绘制） |
| `crates/zdown-app/src/menu.rs` | 无需修改（`handle_shortcuts` 已调用 `state.save()`，检查在 save 内部触发） |
| `crates/config/src/lib.rs` | `AppConfig` 新增 `spell_check_enabled: bool` |
| `crates/zdown-app/src/settings_dialog.rs` | 新增"拼写"标签页 |

**关键设计决定**：
- `source_view.rs` 已接收 `state: &mut EditorState`，直接从 `state.spell_errors` 读取错误列表，无需改函数签名
- `source_view.rs` 当前参数 `app_config: &config::ImageHostingConfig` 仅用于图片粘贴，不变动
- 拼写触发在 `EditorState::save()`/`save_as()` 成功保存后自动执行，`menu.rs` 无需感知

---

## 3. 数据流

### 3.1 保存时检查流程

```
用户按 Ctrl+S 或点击"保存"菜单
    │
    ▼
menu.rs: handle_shortcuts() 或 show_menu()
    │
    └─ state.save() / state.save_as()
           │
           ▼
       EditorState::save() / save_as()
           │
           ├─ 1. workspace.save_to(path, &doc)
           ├─ 2. editor.mark_saved()
           │
           └─ 3. self.run_spell_check(config)
                  │
                  └─ if config.spell_check_enabled:
                         self.spell_errors = self.spell_checker.check(&text)
                     else:
                         self.spell_errors.clear()
    │
    ▼
source_view 下一帧重绘
    render_text_with_cursor() 读取 state.spell_errors
    → 在错误单词所在行下方绘制红色波浪线
```

### 3.2 开关切换流程

```
用户打开设置 → "拼写"标签页 → 取消勾选 → 保存
    │
    ▼
AppConfig::spell_check_enabled = false
AppConfig::save() → config.toml
    │
    ▼
下次保存时：EditorState::save() 检查 config.spell_check_enabled
    → false → self.spell_errors.clear()
    → 下次渲染 source_view 不绘制波浪线
```

**注意**：`EditorState` 需要访问 `AppConfig::spell_check_enabled` 来判断是否执行检查。
`EditorState::save()` 当前签名不接收 config，需新增 `spell_check_enabled: bool` 参数，
或者由上层（`menu.rs`）在 save 之后单独调用 `state.run_spell_check()`。

推荐方案：在 `menu.rs` 的保存调用之后，由上层触发检查，保持 `EditorState::save()` 不依赖 config：
```rust
// menu.rs handle_shortcuts → Ctrl+S
let _ = state.save();
state.run_spell_check(app_config.spell_check_enabled);
```

---

## 4. UI 设计

### 4.1 设置 — "拼写"标签页

在现有设置对话框新增第四个标签：

```
┌─────────────────────────────────────────────────────────┐
│  [样式]  [字体]  [图片]  [拼写]                          │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ☑ 启用英文拼写检查                                      │
│                                                         │
│  拼写检查在保存文件时自动执行。                             │
│  错误单词将以红色波浪下划线标记。                           │
│                                                         │
│  词典：English (United States) — en_US                   │
│                                                         │
│                              [保存]  [取消]               │
└─────────────────────────────────────────────────────────┘
```

- Checkbox 绑定 `dialog.spell_check_buffer: bool`
- 静态提示文字说明检查时机和标记方式
- 显示当前使用的词典信息

### 4.2 波浪下划线渲染

`source_view.rs` 使用 `render_text_with_cursor()` 做逐行自定义绘制（非 TextEdit）。
拼写波浪线在每行文本绘制完成后叠加。

**绘制顺序（render_text_with_cursor 内）**：
```
  1. 匹配高亮背景（搜索功能）
  2. 语法高亮字符逐个绘制
  3. 光标矩形
  4. 拼写错误波浪线 ← 新增，在光标之后绘制（最上层）
```

**逐行位置计算**：
由于 `render_text_with_cursor` 已逐行逐字符计算 x 坐标，可直接利用：

1. 从 `SpellError.span` 获取字节范围
2. 按行分组：通过 `str::lines()` 的字节偏移定位错误单词所在行
3. 在对应行内，计算错误单词起始/结束的 x 像素位置（= 行前缀字符宽度之和）
4. 在该行 rect 下方绘制波浪线

**波浪线算法**（与 TextEdit 版本相同）：
```rust
fn paint_squiggly_underline(
    painter: &egui::Painter,
    start_pos: egui::Pos2,  // 单词起始像素（行左 + 前缀宽度, 行底）
    end_pos: egui::Pos2,    // 单词结束像素
    color: egui::Color32,   // 错误红色
) {
    let step = 3.0;
    let amp = 2.0;
    let y_base = start_pos.y + 2.0;
    let mut points = Vec::new();
    let mut x = start_pos.x;
    while x < end_pos.x {
        let phase = ((x - start_pos.x) / step) as i32;
        let y = y_base + if phase % 2 == 0 { amp } else { -amp };
        points.push(egui::pos2(x, y));
        x += step;
    }
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.0, color)));
}
```

### 4.3 源码视图改动

**函数签名不变**。`show_source_view` 已接收 `state: &mut EditorState`，
直接从 `state.spell_errors` 读取错误列表并传给 `render_text_with_cursor`。

`render_text_with_cursor` 函数新增参数：
```rust
fn render_text_with_cursor(
    ui: &mut egui::Ui,
    src: &str,
    cursor: Cursor,
    highlighter: Option<&SourceHighlighter>,
    search: &SearchState,
    spell_errors: &[SpellError],  // 新增
) {
```

渲染流程变为：
```
1. 逐行遍历
2. 绘制匹配高亮背景（搜索功能）
3. 绘制语法高亮字符
4. 绘制光标矩形
5. 若 !spell_errors.is_empty()：遍历本行的错误，绘制波浪线
```

---

## 5. 配置持久化

### 5.1 AppConfig 新增字段

```rust
pub struct AppConfig {
    // ... 现有字段 ...
    /// 拼写检查开关。默认启用。
    #[serde(default = "default_spell_check")]
    pub spell_check_enabled: bool,
}

fn default_spell_check() -> bool { true }
```

### 5.2 config.toml 格式

```toml
custom_css = "h1 { color: red; }"
theme = "dark"

[image_hosting]
default_strategy = "Local"
local_dir = "images"

[editor_font]
family = "monospace"
size = 14.0

spell_check_enabled = true     # 新增
```

---

## 6. 边界情况

| 场景 | 处理 |
|------|------|
| 空文档 | check() 返回空 Vec，不绘制 |
| 文档无拼写错误 | check() 返回空 Vec，不绘制 |
| 用户未保存（新文档） | 无 spell_errors，不绘制 |
| 禁用拼写检查后重新启用 | 不清空上次检查结果，立即恢复显示 |
| 切换标签页 | spell_errors 随 EditorState 切换（多标签各有独立状态） |
| 编辑器仅含代码块 | check() 跳过所有代码块，返回空 |
| 词典加载失败 | 打印 tracing::warn，spell_checker 降级为总是返回空的 fallback |
| 超长行 | nuspell 逐行处理，性能可接受 |
| 换行符中间的单词 | 拆分 token 时按空白字符处理，跨行单词不识别 |

---

## 7. 测试策略

### 7.1 单元测试（spellcheck crate）

- `check_word("hello") → true`
- `check_word("helo") → false`
- `check("hello world") → Vec::new()`（全部正确）
- `check("helo wrld") → [SpellError("helo", ...), SpellError("wrld", ...)]`
- `check("123 456") → Vec::new()`（跳过数字）
- `check("a") → Vec::new()`（跳过单字符）
- `check("```\nhelo\n```\nhelo")` — 仅标记第二个 helo（代码块内的跳过）
- `check("`helo` helo")` — 仅标记第二个 helo（行内代码跳过）

### 7.2 集成测试

- 拼写检查开关持久化：修改开关 → 保存 → 重启 → 读取状态正确
- 保存触发检查：输入错误单词 → Ctrl+S → spell_errors 非空
- 禁用后保存不检查：关闭开关 → Ctrl+S → spell_errors 为空
- 设置对话框标签页渲染正常

---

## 8. 文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/spellcheck/Cargo.toml` | 新增 | crate 配置 |
| `crates/spellcheck/src/lib.rs` | 新增 | SpellChecker + SpellError 公开接口 |
| `crates/spellcheck/src/checker.rs` | 新增 | nuspell 封装，check() 实现 |
| `crates/spellcheck/src/dict/en_US.aff` | 新增 | Hunspell 词缀文件 |
| `crates/spellcheck/src/dict/en_US.dic` | 新增 | Hunspell 词表文件 |
| `crates/zdown-app/Cargo.toml` | 修改 | 添加 spellcheck 依赖 |
| `crates/config/src/lib.rs` | 修改 | AppConfig 新增 spell_check_enabled |
| `crates/zdown-app/src/editor_state.rs` | 修改 | EditorState 新增 spell_checker + spell_errors |
| `crates/zdown-app/src/source_view.rs` | 修改 | 渲染波浪下划线 |
| `crates/zdown-app/src/menu.rs` | 修改 | 保存流程触发检查 |
| `crates/zdown-app/src/settings_dialog.rs` | 修改 | 新增"拼写"标签页 |
