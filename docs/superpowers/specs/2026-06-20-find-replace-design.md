# 查找替换功能 — 设计规格

**日期**：2026-06-20
**状态**：已批准

---

## 1. 功能概述

为 zdown 编辑器添加查找和替换功能。用户通过 `Ctrl+F` 激活搜索栏，在源码视图中实时搜索并高亮匹配项，支持导航和替换。

### 1.1 核心需求

- Ctrl+F 打开搜索栏（顶部浮动）
- 实时文本搜索，高亮所有匹配
- 区分大小写、全词匹配选项
- 上/下一个匹配导航
- 替换当前匹配、替换全部
- Esc 关闭搜索栏
- 仅源码视图支持（后续扩展到其他视图）

### 1.2 非需求（本次不实现）

- 正则表达式搜索
- 预览/Hybrid 视图中的搜索高亮
- 跨行匹配
- 搜索历史记录

---

## 2. 架构设计

### 2.1 模块划分

```
zdown-app/src/
  ├── search.rs        ← 新增：纯搜索逻辑
  ├── main.rs          ← 修改：SearchState、Ctrl+F、搜索栏渲染
  ├── source_view.rs   ← 修改：接收 SearchState、匹配高亮
  ├── menu.rs          ← 修改：Ctrl+F 快捷键
  └── input.rs         ← 不修改
```

### 2.2 SearchEngine（search.rs）

纯逻辑模块，不依赖 egui 或 editor_engine。输入文本字符串，返回匹配位置列表。

```rust
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
}

pub struct Match {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
}

pub struct SearchEngine;

impl SearchEngine {
    pub fn find_all(text: &str, query: &str, opts: &SearchOptions) -> Vec<Match>;
}
```

**算法**：
1. 若 `query` 为空，返回空列表
2. 逐行遍历文本
3. 若 `case_sensitive` 为 false，将行和 query 转为小写比较
4. 使用 `str::find` 迭代查找当前行中所有匹配
5. 若 `whole_word` 为 true，检查匹配前后字符是否为单词边界（非字母数字或行首/尾）
6. 收集所有 `Match { line, col_start, col_end }`

### 2.3 SearchState（main.rs）

UI 状态结构体，由 ZdownApp 持有。

```rust
struct SearchState {
    visible: bool,
    query: String,
    replace: String,
    case_sensitive: bool,
    whole_word: bool,
    matches: Vec<Match>,
    current_match: Option<usize>,
    focus_search: bool,
}
```

**状态转换**：
- `visible: false` → `Ctrl+F` → `visible: true, focus_search: true`
- `visible: true` → `Esc` → `visible: false, matches.clear()`
- `query` 变化 → 重新 `find_all` → 更新 `matches` 和 `current_match`
- `case_sensitive`/`whole_word` 变化 → 重新 `find_all`

### 2.4 集成点

| 文件 | 改动 |
|------|------|
| `search.rs` | 新增 SearchEngine + Match + SearchOptions |
| `main.rs` | ZdownApp 添加 `search: SearchState` 字段；`ui()` 中添加 Ctrl+F 处理、搜索栏渲染、替换快捷键 |
| `source_view.rs` | `show_source_view` 新增 `search: &SearchState` 参数；`render_text_with_cursor` 叠加匹配高亮 |
| `menu.rs` | `handle_shortcuts` 添加 Ctrl+F → 切换 `search.visible` |

---

## 3. UI 设计

### 3.1 搜索栏布局

搜索栏在 CentralPanel 顶部，源码视图内容上方。仅当 `search.visible == true` 时渲染。

```
┌─────────────────────────────────────────────────────────┐
│ 🔍 查找: [____________] 2/5  [Aa] [ab|] [←] [→] [✕]  │
│ 🔄 替换: [____________]              [替换] [全部]      │
└─────────────────────────────────────────────────────────┘
```

- **查找行**：标签 + 输入框 + 匹配计数 + 选项按钮 + 导航按钮 + 关闭
- **替换行**：标签 + 输入框 + 替换/全部按钮

### 3.2 匹配高亮

在源码编辑区叠加背景色：
- **当前匹配**：`#d4850b`（橙色），更醒目
- **其他匹配**：`#6b4c12`（暗黄色）
- 光标仍然绘制（在匹配高亮之上）

绘制顺序：语法高亮文本 → 匹配背景 → 光标矩形

### 3.3 快捷键

| 快捷键 | 上下文 | 行为 |
|--------|--------|------|
| `Ctrl+F` | 全局 | 打开搜索栏，焦点进入查找框 |
| `Enter` | 查找框有焦点 | 跳到下一个匹配 |
| `Shift+Enter` | 查找框有焦点 | 跳到上一个匹配 |
| `Esc` | 搜索栏可见 | 关闭搜索栏 |

### 3.4 按钮行为

| 按钮 | 行为 |
|------|------|
| `Aa` | 切换区分大小写（toggle，按下状态有视觉反馈） |
| `ab\|` | 切换全词匹配（toggle） |
| `←` | 上一个匹配，光标跟随 |
| `→` | 下一个匹配，光标跟随 |
| `替换` | 替换当前匹配为 replace 文本，跳到下一个 |
| `全部` | 替换所有匹配，显示替换数量 |
| `✕` | 关闭搜索栏 |

---

## 4. 数据流

### 4.1 搜索流程

```
Ctrl+F
  → search.visible = true
  → search.focus_search = true
  → UI 渲染搜索栏，查找输入框获得焦点

用户输入 'foo'
  → search.query = 'foo'
  → SearchEngine::find_all(editor_text, 'foo', opts)
  → search.matches = [Match(2,3,6), Match(4,8,11), Match(5,1,4)]
  → search.current_match = Some(0)
  → 编辑器光标跳至 Match(2,3,6)
  → source_view 高亮所有匹配，橙色高亮当前匹配

用户按 Enter
  → current_match = 1
  → 光标跳至 Match(4,8,11)
```

### 4.2 替换流程

```
用户输入替换文本 'bar'
  → search.replace = 'bar'

用户点 "替换"
  → 对 matches[current_match] 执行 Command::Replace
  → 重新 find_all（文本已变化）
  → 更新 matches / current_match
  → 如果还有匹配，自动跳到下一个

用户点 "全部"
  → 遍历所有 matches，逐个执行 Command::Replace
  → 注意：替换可能改变文本长度，需从后往前替换
  → 重新 find_all
  → 状态栏显示 "已替换 N 处"
```

---

## 5. 边界情况

| 场景 | 处理 |
|------|------|
| 空查询 | 返回空匹配列表，不高亮，显示 "0/0" |
| 无匹配结果 | 显示 "0/0"，输入框边框变红（可选） |
| 查询变化 | 重新搜索，尝试保持 current_match 在相近位置 |
| 替换后文本变化 | 所有匹配位置重新计算 |
| 文档为空 | 搜索栏仍可打开，但 find_all 返回空 |
| 快速输入 | 每帧渲染时根据当前 query 搜索（无需防抖） |
| 切换标签页 | 搜索状态是否保留？——不保留，切换标签时关闭搜索 |
| 切换视图模式 | 搜索状态是否保留？——保留搜索栏，高亮跟随视图 |

---

## 6. 测试策略

### 6.1 单元测试（search.rs）

- 空查询返回空
- 基本匹配（单行单匹配）
- 多匹配（单行多匹配）
- 多行匹配
- 区分大小写：开启/关闭
- 全词匹配：开启/关闭
- 无匹配返回空
- Unicode 字符匹配

### 6.2 集成测试

- Ctrl+F 打开搜索栏
- 输入搜索词 → 高亮出现
- 导航上下 → 光标移动
- 替换单个 → 文本变化、高亮更新
- Esc 关闭 → 高亮消失

---

## 7. 文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/zdown-app/src/search.rs` | 新增 | SearchEngine 纯逻辑 |
| `crates/zdown-app/src/main.rs` | 修改 | SearchState、Ctrl+F、搜索栏 UI |
| `crates/zdown-app/src/source_view.rs` | 修改 | 匹配高亮绘制 |
| `crates/zdown-app/src/menu.rs` | 修改 | Ctrl+F 快捷键 |
