# 终端面板设计规格

**日期**: 2026-06-21  
**状态**: 已批准  
**目标**: 在 zdown 编辑器内嵌一个完整 ANSI/VT 终端面板

---

## 1. 概述

在编辑器底部集成一个可切换的终端面板，支持完整的 ANSI/VT 终端模拟。用户在编辑 Markdown 的同时可以直接运行 shell 命令。

### 功能范围

- 跨平台 PTY（Windows ConPTY / Unix pty）
- 完整 ANSI/VT 转义序列支持（颜色、光标控制、清屏等）
- 可拖拽调节面板高度
- `Ctrl+`` 快捷键切换终端显隐
- 6 套预置色彩主题
- 终端内文本选择和复制
- 鼠标报告（支持 vim、htop 等 TUI 程序）
- 10,000 行 scrollback
- 面板关闭时保持 shell 进程（不因隐藏而 kill）

### 不在此范围

- 多标签终端（后续迭代）
- 终端内粘贴（需单独处理 bracketed paste，后续迭代）
- 自定义终端字体（跟随编辑器 monospace 字体）
- 终端超链接点击

---

## 2. 技术方案

**策略**: Fork `egui_term` (MIT, ~2000 行) 并升级其渲染层到 egui 0.34。

### 依赖

| 库 | 版本 | 用途 |
|---|---|---|
| `alacritty_terminal` | 0.26 | VTE 解析器 + Grid 存储 + Term 状态机 |
| `portable-pty` | 0.9 | 跨平台 PTY 创建 (Unix pty / Windows ConPTY) |
| `egui` | 0.34 (workspace) | UI 渲染 |

### egui 0.34 渲染适配

egui_term 原版使用 `Shape::text()` 逐 cell 绘制字符，此 API 在 egui 0.34 已被移除。适配方式：

**旧 (egui 0.31)**:
```rust
Shape::text(font, pos, galley, color)
Shape::rect_filled(rect, rounding, bg)
```

**新 (egui 0.34)**:
```rust
// 行级批量布局
let mut job = LayoutJob::default();
for cell in row.cells() {
    job.append(&cell.character, 0.0, TextFormat {
        font_id: mono_font,
        color: fg_color(cell.fg),
        background: bg_color(cell.bg),
        ..Default::default()
    });
}
let galley = ui.fonts(|f| f.layout_job(job));
ui.painter().galley(row_pos, galley, Color32::WHITE);
```

行级 galley 批量渲染（N 行 = N 个 galley），比逐 cell 渲染更高效。

---

## 3. 架构

### 3.1 模块划分

新 crate: `crates/terminal_panel/`

```
crates/terminal_panel/
├── lib.rs          # TerminalPanel 状态机、公开 API
├── backend.rs      # PTY 生命周期、I/O 线程、Term 集成
├── view.rs         # egui Widget 实现、grid→galley 渲染
├── bindings.rs     # 键盘/鼠标事件 → ANSI 转义序列转换
└── theme.rs        # 6 套 ANSI 16 色调色板
```

### 3.2 线程模型

```
主线程 (egui)                    后台线程 (PTY I/O)
─────────────────                ────────────────────
TerminalPanel                      loop {
  ├─ TerminalBackend                 master.read()  → Term
  │   ├─ Term<EventProxy>            Term.advance() → events
  │   ├─ EventLoop                   ctx.request_repaint()
  │   └─ Notifier                  }
  ├─ handle_events() 每帧
  ├─ write(input)    按键时
  └─ resize()        面板高度变化时
```

通信: `alacritty_terminal::event::Notifier` 在 PTY 有新输出时通知，通过 `egui::Context::request_repaint()` 触发 UI 重绘。无轮询。

### 3.3 集成点

- `ZdownApp` 新增字段: `terminal: Option<TerminalPanel>`（首次打开时延迟初始化）
- 布局: `egui::TopBottomPanel::bottom("terminal").resizable(true)` 内嵌于 CentralPanel
- 快捷键: 菜单栏 "视图 → 终端" + `Ctrl+`` 切换显隐
- 菜单/快捷键处理在 `menu.rs` 中已有集中管理

---

## 4. 输入处理

### 4.1 键盘

| 输入类型 | 发送到 PTY |
|---|---|
| 可打印字符 (a-z, 0-9, 符号) | UTF-8 字节原样 |
| 特殊键 (Enter, Tab, BS, Esc, Arrow...) | 对应 ANSI 序列 (`\r`, `\x7f`, `\x1b[A` ...) |
| Ctrl 组合 (Ctrl+C, Ctrl+D...) | 控制字符 (0x03, 0x04...) |

`Ctrl+`` 被编辑器拦截用于切换终端显隐，不发送到 PTY。

### 4.2 鼠标

当终端程序启用鼠标报告时（SGR 模式），发送:
- 左/中/右键: `\x1b[<b;col;rowM`
- 释放: `\x1b[<b;col;rowm`
- 滚轮: `\x1b[<64/65;col;rowM`

文本选择（鼠标拖动）在 UI 层处理，不走 PTY，选择后复制到剪贴板。

### 4.3 Shell 选择

| 平台 | 选择逻辑 |
|------|---------|
| Windows | `powershell.exe`（ConPTY 要求 Win10 1809+）|
| Linux | `$SHELL` 环境变量，fallback `/bin/bash` |
| macOS | `$SHELL` 环境变量，fallback `/bin/zsh` |

---

## 5. 视觉规格

### 5.1 色彩主题

6 套预置 ANSI 16 色调色板:

| 主题名 | 风格 |
|--------|------|
| `dracula` | 暗色，紫色调 |
| `solarized-dark` | 暗色，黄/蓝 |
| `solarized-light` | 亮色 |
| `one-dark` | 暗色，Atom/VSCode 风格 |
| `tokyo-night` | 暗色，蓝紫调 |
| `monokai` | 暗色，Sublime 经典 |

默认跟随编辑器主题 (`one-dark` for Dark, `solarized-light` for Light)。

### 5.2 字体

- 使用 egui 当前 monospace 字体（与编辑器源码视图一致）
- 单元格大小: `glyph_width × row_height`，通过 `ctx.fonts(|f| f.glyph_width(...))` 获取

### 5.3 光标

- 默认: 闪烁方块
- 支持竖线（beam）和下划线（underline）—— 当终端程序通过 DECSET 序列请求时
- 渲染方式: 在对应 cell 上覆盖绘制矩形/竖线

### 5.4 滚动与回滚

- `alacritty_terminal::Term` 自带 scrollback
- 右侧绘制 egui 原生滚动条
- `Shift+PgUp` / `Shift+PgDown` 翻阅回滚历史
- 默认 scrollback: 10,000 行

---

## 6. 错误处理

- PTY 创建失败 → tracing::error 日志 + 终端面板显示 "Failed to spawn shell" 错误信息
- PTY 进程意外退出 → 终端面板显示 "Process exited with code N" + "按 Enter 重新启动"
- 字体度量获取失败 → fallback 到固定宽度 8px
- resize 失败 → 忽略（TUI 程序最多显示异常，不影响编辑器稳定性）
- 所有 PTY 写入错误 → tracing::warn，不 panic

---

## 7. 测试策略

### 单元测试
- `bindings.rs`: 键盘映射正确性（字符 → ANSI 序列）
- `theme.rs`: 16 色调色板颜色值一致性
- `view.rs`: Grid 内容 → LayoutJob 转换逻辑

### 集成测试
- `TerminalPanel::new()` spawn 后 `is_alive()` 返回 true
- 写入 `echo hello` 后读取到 `hello` 输出（端到端）
- resize 后 Term 的 rows/cols 更新正确
- 关闭面板不 kill 进程，重新打开仍可用
- 面板显隐切换不影响编辑器状态

### 手动测试
- Windows: PowerShell、cmd 基本操作
- TUI 程序: vim、htop（需 ConPTY 支持）
- ANSI 颜色: `echo -e "\e[31mred\e[0m"`

---

## 8. 实现依赖

此功能**不依赖**以下尚未完成的功能：
- 插件系统
- AI 续写
- HTML 渲染改进

可以在当前代码基础上独立实现。

---

## 9. 后续迭代

- 多标签终端
- Bracketed paste 支持
- 终端内超链接点击
- 自定义终端字体
- 终端搜索
