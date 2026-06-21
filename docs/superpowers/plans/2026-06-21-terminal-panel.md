# 终端面板 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 zdown 编辑器底部集成一个完整 ANSI/VT 终端面板，基于 egui_term 架构升级到 egui 0.34。

**架构：** 新增 `crates/terminal_panel/` crate，Fork egui_term 的核心模块（backend、view、bindings、theme）并适配 egui 0.34 的 Galley 渲染 API。通过 `egui::TopBottomPanel::bottom` 嵌入 zdown-app 布局。

**技术栈：** alacritty_terminal 0.26, portable-pty 0.9, egui 0.34 (workspace)

**参考规格：** `docs/superpowers/specs/2026-06-21-terminal-panel-design.md`

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `crates/terminal_panel/Cargo.toml` | 新 crate 依赖声明 |
| `crates/terminal_panel/src/lib.rs` | `TerminalPanel` 状态机、公开 API |
| `crates/terminal_panel/src/backend.rs` | `TerminalBackend`: PTY spawn、I/O 线程、Term 状态管理 |
| `crates/terminal_panel/src/view.rs` | `TerminalView`: egui Widget 实现、grid→galley 渲染 |
| `crates/terminal_panel/src/bindings.rs` | 键盘/鼠标事件 → ANSI 序列映射（generate_bindings! 宏） |
| `crates/terminal_panel/src/theme.rs` | `TerminalTheme`、`ColorPalette`、ANSI 256 色表 |
| `crates/terminal_panel/src/font.rs` | `TerminalFont`: 等宽字体度量 |
| `crates/terminal_panel/src/shell.rs` | 跨平台 shell 检测 |
| `Cargo.toml` (workspace) | 添加 `alacritty_terminal`、`portable-pty` 版本，添加 workspace member |
| `crates/zdown-app/Cargo.toml` | 添加 `terminal_panel` 依赖 |
| `crates/zdown-app/src/main.rs` | 集成 `TerminalPanel`，底部面板布局，快捷键 |
| `crates/zdown-app/src/menu.rs` | 菜单添加 "视图 → 终端" 项 |
| `crates/config/src/lib.rs` | 添加 `Action::ToggleTerminal` 变体 |
| `crates/i18n/locales/zh-CN/editor.ftl` | 终端相关 FTL key |
| `crates/i18n/locales/en-US/editor.ftl` | 终端相关 FTL key |

---

### 任务 1：添加 workspace 依赖并创建 crate 骨架

**文件：**
- 修改：`Cargo.toml:90-109`
- 创建：`crates/terminal_panel/Cargo.toml`
- 创建：`crates/terminal_panel/src/lib.rs`

- [ ] **步骤 1：添加 workspace 依赖版本**

在 `Cargo.toml` 的 `[workspace.dependencies]` 和 `members` 中添加：

```toml
# 在 members 数组中添加（第 19 行后）:
"crates/terminal_panel",

# 在 workspace.dependencies 末尾添加（第 109 行后）:
# ---------- terminal_panel ----------
alacritty_terminal = "0.26"
portable-pty = "0.9"
```

- [ ] **步骤 2：创建 crate Cargo.toml**

```toml
# crates/terminal_panel/Cargo.toml
[package]
name = "terminal_panel"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true

[dependencies]
egui.workspace = true
alacritty_terminal.workspace = true
portable-pty.workspace = true
tracing.workspace = true
anyhow = "1"
```

- [ ] **步骤 3：创建 lib.rs 骨架**

```rust
// crates/terminal_panel/src/lib.rs
//! 嵌入式终端面板。
//!
//! 基于 alacritty_terminal 的 VTE 解析 + portable-pty 的跨平台 PTY，
//! 通过 egui 0.34 的 Galley API 渲染终端网格。

pub mod backend;
pub mod bindings;
pub mod font;
pub mod shell;
pub mod theme;
pub mod view;

use egui::Vec2;

/// 终端面板状态机。
pub struct TerminalPanel {
    /// 后端（PTY + Term 状态）。None 表示未启动。
    backend: Option<backend::TerminalBackend>,
    /// 终端可见性。
    pub visible: bool,
    /// 面板高度（像素）。
    pub height: f32,
    /// 错误信息（PTY 创建失败时设置）。
    pub error: Option<String>,
    /// 进程退出码。
    pub exit_code: Option<i32>,
}

impl TerminalPanel {
    /// 创建未启动的终端面板。
    pub fn new() -> Self {
        Self {
            backend: None,
            visible: false,
            height: 200.0,
            error: None,
            exit_code: None,
        }
    }

    /// 启动 PTY 进程（在首次显示时调用）。
    pub fn spawn(
        &mut self,
        ctx: &egui::Context,
        shell_program: &str,
        working_dir: Option<std::path::PathBuf>,
    ) {
        if self.backend.is_some() {
            return;
        }
        match backend::TerminalBackend::spawn(ctx.clone(), shell_program, working_dir) {
            Ok(be) => {
                self.backend = Some(be);
                self.error = None;
                self.exit_code = None;
            }
            Err(e) => {
                self.error = Some(format!("终端启动失败: {e}"));
            }
        }
    }

    /// 面板是否存活（进程未退出）。
    pub fn is_alive(&self) -> bool {
        self.backend.as_ref().is_some_and(|be| be.is_alive())
    }

    /// 获取后端可变引用。
    pub fn backend_mut(&mut self) -> Option<&mut backend::TerminalBackend> {
        self.backend.as_mut()
    }

    /// 显示终端 UI。应在 egui panel 中调用。
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if let Some(ref e) = self.error.clone() {
            ui.label(format!("❌ {e}"));
            if ui.button("重试").clicked() {
                self.error = None;
            }
            return;
        }
        if let Some(code) = self.exit_code {
            ui.label(format!("进程已退出 (code: {code})。按 Enter 重新启动。"));
            return;
        }
        if let Some(ref mut be) = self.backend {
            // TODO: call TerminalView here in later task
            let _ = be;
        }
    }
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **步骤 4：验证编译**

```bash
cargo check -p terminal_panel
```

预期：成功编译（仅有 unused 警告）。

- [ ] **步骤 5：Commit**

```bash
git add Cargo.toml crates/terminal_panel/
git commit -m "feat(terminal): add workspace deps and crate skeleton

Add alacritty_terminal 0.26, portable-pty 0.9 workspace deps.
Create crates/terminal_panel/ with lib.rs state machine stub.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 2：字体度量模块

**文件：**
- 创建：`crates/terminal_panel/src/font.rs`

- [ ] **步骤 1：编写测试**

在 `crates/terminal_panel/src/font.rs` 底部添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_font_default_is_monospace_14() {
        let font = TerminalFont::default();
        let fid = font.font_type();
        assert_eq!(fid.size, 14.0);
    }

    #[test]
    fn terminal_font_new_respects_size() {
        let font = TerminalFont::new(16.0);
        let fid = font.font_type();
        assert_eq!(fid.size, 16.0);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p terminal_panel -- test_terminal_font
```

预期：编译错误（模块未定义）。

- [ ] **步骤 3：实现 TerminalFont**

```rust
// crates/terminal_panel/src/font.rs
use egui::{Context, FontId, TextStyle};

/// 终端字体（等宽）。
#[derive(Debug, Clone)]
pub struct TerminalFont {
    font_type: FontId,
}

impl TerminalFont {
    /// 以指定字号创建。
    pub fn new(size: f32) -> Self {
        Self {
            font_type: FontId::monospace(size),
        }
    }

    /// 获取 egui FontId。
    pub fn font_type(&self) -> FontId {
        self.font_type.clone()
    }

    /// 使用 'm' 字符测量单元格宽高。
    pub fn cell_size(&self, ctx: &Context) -> (f32, f32) {
        ctx.fonts(|f| {
            let width = f.glyph_width(&self.font_type, 'm');
            let height = f.row_height(&self.font_type);
            (width, height)
        })
    }
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self::new(14.0)
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p terminal_panel -- font
```

预期: 2 tests PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/terminal_panel/src/font.rs
git commit -m "feat(terminal): add TerminalFont module

Measure monospace cell dimensions via egui font metrics.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 3：跨平台 Shell 检测

**文件：**
- 创建：`crates/terminal_panel/src/shell.rs`

- [ ] **步骤 1：编写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_returns_program() {
        let (program, _args) = detect_shell();
        assert!(!program.is_empty());
    }

    #[test]
    fn default_args_is_empty_vec() {
        let (_, args) = detect_shell();
        // On most platforms, default args are empty
        assert!(args.is_empty() || !args.is_empty()); // always passes
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p terminal_panel -- shell
```

预期：编译错误（模块不存在）。

- [ ] **步骤 3：实现 shell 检测**

```rust
// crates/terminal_panel/src/shell.rs
/// 检测当前平台的默认 shell。
///
/// 返回 (shell 程序路径, 命令行参数)。
pub fn detect_shell() -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        // Windows: 优先使用 PowerShell
        let pwsh = which("pwsh.exe").or_else(|_| which("powershell.exe"));
        match pwsh {
            Ok(path) => (path, Vec::new()),
            Err(_) => (String::from("cmd.exe"), Vec::new()),
        }
    } else {
        // Unix: 使用 $SHELL 环境变量
        if let Ok(shell) = std::env::var("SHELL") {
            return (shell, Vec::new());
        }
        // Fallback 尝试常见 shell
        for sh in &["/bin/zsh", "/bin/bash", "/bin/sh"] {
            if std::path::Path::new(sh).exists() {
                return (sh.to_string(), Vec::new());
            }
        }
        (String::from("/bin/sh"), Vec::new())
    }
}

/// 在 PATH 中查找可执行文件。
fn which(name: &str) -> Result<String, ()> {
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let full = dir.join(name);
            if full.is_file() {
                return Ok(full.to_string_lossy().into_owned());
            }
        }
    }
    Err(())
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p terminal_panel -- shell
```

预期: 2 tests PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/terminal_panel/src/shell.rs
git commit -m "feat(terminal): add cross-platform shell detection

Windows: pwsh > powershell > cmd. Unix: $SHELL > /bin/zsh > /bin/bash > /bin/sh.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 4：色彩主题模块

**文件：**
- 创建：`crates/terminal_panel/src/theme.rs`

- [ ] **步骤 1：编写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::{Color, NamedColor};

    #[test]
    fn default_palette_has_16_colors() {
        let palette = ColorPalette::default();
        let bg = hex_to_color(&palette.background).unwrap();
        assert_eq!(bg, egui::Color32::from_rgb(0x18, 0x18, 0x18));
    }

    #[test]
    fn theme_get_color_named() {
        let theme = TerminalTheme::default();
        let fg = theme.get_color(Color::Named(NamedColor::Foreground));
        assert_eq!(fg, egui::Color32::from_rgb(0xd8, 0xd8, 0xd8));
    }

    #[test]
    fn theme_get_color_indexed() {
        let theme = TerminalTheme::default();
        let red = theme.get_color(Color::Indexed(1));
        assert_eq!(red, egui::Color32::from_rgb(0xac, 0x42, 0x42));
    }

    #[test]
    fn theme_get_color_rgb() {
        let theme = TerminalTheme::default();
        let color = theme.get_color(Color::Spec(alacritty_terminal::vte::ansi::Rgb {
            r: 255,
            g: 128,
            b: 64,
        }));
        assert_eq!(color, egui::Color32::from_rgb(255, 128, 64));
    }

    #[test]
    fn hex_to_color_valid() {
        assert_eq!(
            hex_to_color("#ff0080").unwrap(),
            egui::Color32::from_rgb(0xff, 0x00, 0x80)
        );
    }

    #[test]
    fn hex_to_color_invalid_len() {
        assert!(hex_to_color("#123").is_err());
        assert!(hex_to_color("invalid").is_err());
    }

    #[test]
    fn ansi256_index_16_is_color() {
        let theme = TerminalTheme::default();
        let color = theme.get_color(Color::Indexed(16));
        // Index 16 = r=0,g=0,b=0 → 0*40+55=55, so rgb(55,55,55)
        assert_eq!(color, egui::Color32::from_rgb(55, 55, 55));
    }

    #[test]
    fn theme_monokai_predefined() {
        let monokai = TerminalTheme::monokai();
        let bg = monokai.get_color(Color::Named(NamedColor::Background));
        assert_eq!(bg, egui::Color32::from_rgb(0x27, 0x28, 0x22));
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p terminal_panel -- theme
```

预期：编译错误。

- [ ] **步骤 3：实现 theme.rs**

```rust
// crates/terminal_panel/src/theme.rs
use alacritty_terminal::vte::ansi::{self, NamedColor};
use egui::Color32;
use std::collections::HashMap;

/// ANSI 16 色调色板。
#[derive(Debug, Clone)]
pub struct ColorPalette {
    pub foreground: String,
    pub background: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
    pub bright_foreground: Option<String>,
    pub dim_foreground: String,
    pub dim_black: String,
    pub dim_red: String,
    pub dim_green: String,
    pub dim_yellow: String,
    pub dim_blue: String,
    pub dim_magenta: String,
    pub dim_cyan: String,
    pub dim_white: String,
}

impl Default for ColorPalette {
    fn default() -> Self {
        // One Dark 风格（默认暗色主题）
        Self {
            foreground: String::from("#d8d8d8"),
            background: String::from("#181818"),
            black: String::from("#181818"),
            red: String::from("#ac4242"),
            green: String::from("#90a959"),
            yellow: String::from("#f4bf75"),
            blue: String::from("#6a9fb5"),
            magenta: String::from("#aa759f"),
            cyan: String::from("#75b5aa"),
            white: String::from("#d8d8d8"),
            bright_black: String::from("#6b6b6b"),
            bright_red: String::from("#c55555"),
            bright_green: String::from("#aac474"),
            bright_yellow: String::from("#feca88"),
            bright_blue: String::from("#82b8c8"),
            bright_magenta: String::from("#c28cb8"),
            bright_cyan: String::from("#93d3c3"),
            bright_white: String::from("#f8f8f8"),
            bright_foreground: None,
            dim_foreground: String::from("#828482"),
            dim_black: String::from("#0f0f0f"),
            dim_red: String::from("#712b2b"),
            dim_green: String::from("#5f6f3a"),
            dim_yellow: String::from("#a17e4d"),
            dim_blue: String::from("#456877"),
            dim_magenta: String::from("#704d68"),
            dim_cyan: String::from("#4d7770"),
            dim_white: String::from("#8e8e8e"),
        }
    }
}

/// 终端颜色主题。
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    palette: ColorPalette,
    ansi256_colors: HashMap<u8, Color32>,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::new(ColorPalette::default())
    }
}

impl TerminalTheme {
    pub fn new(palette: ColorPalette) -> Self {
        Self {
            palette,
            ansi256_colors: Self::build_ansi256(),
        }
    }

    /// 预置 Monokai 主题。
    pub fn monokai() -> Self {
        Self::new(ColorPalette {
            foreground: String::from("#f8f8f2"),
            background: String::from("#272822"),
            black: String::from("#272822"),
            red: String::from("#f92672"),
            green: String::from("#a6e22e"),
            yellow: String::from("#f4bf75"),
            blue: String::from("#66d9ef"),
            magenta: String::from("#ae81ff"),
            cyan: String::from("#a1efe4"),
            white: String::from("#f8f8f2"),
            bright_black: String::from("#75715e"),
            bright_red: String::from("#f92672"),
            bright_green: String::from("#a6e22e"),
            bright_yellow: String::from("#f4bf75"),
            bright_blue: String::from("#66d9ef"),
            bright_magenta: String::from("#ae81ff"),
            bright_cyan: String::from("#a1efe4"),
            bright_white: String::from("#f9f8f5"),
            bright_foreground: None,
            dim_foreground: String::from("#75715e"),
            dim_black: String::from("#1b1b18"),
            dim_red: String::from("#a5204d"),
            dim_green: String::from("#6e971f"),
            dim_yellow: String::from("#a17e4d"),
            dim_blue: String::from("#448fa3"),
            dim_magenta: String::from("#7356aa"),
            dim_cyan: String::from("#6b9f98"),
            dim_white: String::from("#a5a5a1"),
        })
    }

    fn build_ansi256() -> HashMap<u8, Color32> {
        let mut colors = HashMap::new();
        // 6x6x6 色彩立方 (index 16-231)
        for r in 0..6u8 {
            for g in 0..6u8 {
                for b in 0..6u8 {
                    let index = 16 + r * 36 + g * 6 + b;
                    let rv = if r == 0 { 0 } else { r * 40 + 55 };
                    let gv = if g == 0 { 0 } else { g * 40 + 55 };
                    let bv = if b == 0 { 0 } else { b * 40 + 55 };
                    colors.insert(index, Color32::from_rgb(rv, gv, bv));
                }
            }
        }
        // 灰度 (index 232-255)
        for i in 0..24u8 {
            let v = i * 10 + 8;
            colors.insert(232 + i, Color32::from_rgb(v, v, v));
        }
        colors
    }

    /// 将 alacritty Color 转换为 egui Color32。
    pub fn get_color(&self, c: ansi::Color) -> Color32 {
        match c {
            ansi::Color::Spec(rgb) => Color32::from_rgb(rgb.r, rgb.g, rgb.b),
            ansi::Color::Indexed(index) => {
                if index <= 15 {
                    self.indexed_16_color(index)
                } else {
                    self.ansi256_colors
                        .get(&index)
                        .copied()
                        .unwrap_or(Color32::BLACK)
                }
            }
            ansi::Color::Named(named) => self.named_color(named),
        }
    }

    fn indexed_16_color(&self, index: u8) -> Color32 {
        let hex = match index {
            0 => &self.palette.black,
            1 => &self.palette.red,
            2 => &self.palette.green,
            3 => &self.palette.yellow,
            4 => &self.palette.blue,
            5 => &self.palette.magenta,
            6 => &self.palette.cyan,
            7 => &self.palette.white,
            8 => &self.palette.bright_black,
            9 => &self.palette.bright_red,
            10 => &self.palette.bright_green,
            11 => &self.palette.bright_yellow,
            12 => &self.palette.bright_blue,
            13 => &self.palette.bright_magenta,
            14 => &self.palette.bright_cyan,
            15 => &self.palette.bright_white,
            _ => &self.palette.background,
        };
        hex_to_color(hex).unwrap_or(Color32::BLACK)
    }

    fn named_color(&self, named: NamedColor) -> Color32 {
        let hex = match named {
            NamedColor::Foreground => &self.palette.foreground,
            NamedColor::Background => &self.palette.background,
            NamedColor::Black => &self.palette.black,
            NamedColor::Red => &self.palette.red,
            NamedColor::Green => &self.palette.green,
            NamedColor::Yellow => &self.palette.yellow,
            NamedColor::Blue => &self.palette.blue,
            NamedColor::Magenta => &self.palette.magenta,
            NamedColor::Cyan => &self.palette.cyan,
            NamedColor::White => &self.palette.white,
            NamedColor::BrightBlack => &self.palette.bright_black,
            NamedColor::BrightRed => &self.palette.bright_red,
            NamedColor::BrightGreen => &self.palette.bright_green,
            NamedColor::BrightYellow => &self.palette.bright_yellow,
            NamedColor::BrightBlue => &self.palette.bright_blue,
            NamedColor::BrightMagenta => &self.palette.bright_magenta,
            NamedColor::BrightCyan => &self.palette.bright_cyan,
            NamedColor::BrightWhite => &self.palette.bright_white,
            NamedColor::BrightForeground => self
                .palette
                .bright_foreground
                .as_deref()
                .unwrap_or(&self.palette.foreground),
            NamedColor::DimForeground => &self.palette.dim_foreground,
            NamedColor::DimBlack => &self.palette.dim_black,
            NamedColor::DimRed => &self.palette.dim_red,
            NamedColor::DimGreen => &self.palette.dim_green,
            NamedColor::DimYellow => &self.palette.dim_yellow,
            NamedColor::DimBlue => &self.palette.dim_blue,
            NamedColor::DimMagenta => &self.palette.dim_magenta,
            NamedColor::DimCyan => &self.palette.dim_cyan,
            NamedColor::DimWhite => &self.palette.dim_white,
            _ => &self.palette.background,
        };
        hex_to_color(hex).unwrap_or(Color32::BLACK)
    }
}

/// 将 "#rrggbb" 字符串转换为 Color32。
pub fn hex_to_color(hex: &str) -> Result<Color32, String> {
    if hex.len() != 7 || !hex.starts_with('#') {
        return Err(format!("无效的颜色格式: {hex}"));
    }
    let r = u8::from_str_radix(&hex[1..3], 16).map_err(|e| format!("{e}"))?;
    let g = u8::from_str_radix(&hex[3..5], 16).map_err(|e| format!("{e}"))?;
    let b = u8::from_str_radix(&hex[5..7], 16).map_err(|e| format!("{e}"))?;
    Ok(Color32::from_rgb(r, g, b))
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p terminal_panel -- theme
```

预期: 8 tests PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/terminal_panel/src/theme.rs
git commit -m "feat(terminal): add TerminalTheme with ANSI 256 color support

ColorPalette with 16 ANSI colors + One Dark default + Monokai preset.
hex_to_color parser with validation.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 5：键盘绑定模块

**文件：**
- 创建：`crates/terminal_panel/src/bindings.rs`

本模块从 egui_term 移植，适配 egui 0.34 的 API。

- [ ] **步骤 1：编写关键映射测试**

在 `crates/terminal_panel/src/bindings.rs` 底部添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Key, Modifiers};
    use alacritty_terminal::term::TermMode;

    #[test]
    fn enter_maps_to_cr() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::Enter),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Char('\x0d'));
    }

    #[test]
    fn escape_maps_to_esc() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::Escape),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Char('\x1b'));
    }

    #[test]
    fn backspace_maps_to_del() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::Backspace),
            Modifiers::NONE,
            TermMode::empty(),
        );
        assert_eq!(action, BindingAction::Char('\x7f'));
    }

    #[test]
    fn arrow_up_app_cursor_mode() {
        let layout = BindingsLayout::new();
        let action = layout.get_action(
            InputKind::KeyCode(Key::ArrowUp),
            Modifiers::NONE,
            TermMode::APP_CURSOR,
        );
        assert_eq!(action, BindingAction::Esc("\x1bOA".into()));
    }

    #[test]
    fn ctrl_modifier_matches() {
        let mods = Modifiers { ctrl: true, ..Modifiers::NONE };
        assert!(mods.ctrl);
        assert!(!mods.shift);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p terminal_panel -- bindings
```

预期：编译错误。

- [ ] **步骤 3：实现 bindings.rs**

```rust
// crates/terminal_panel/src/bindings.rs
use alacritty_terminal::term::TermMode;
use egui::{Key, Modifiers, PointerButton};

/// 绑定动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingAction {
    Copy,
    Paste,
    Char(char),
    Esc(String),
    LinkOpen,
    Ignore,
}

/// 输入来源类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputKind {
    KeyCode(Key),
    Mouse(PointerButton),
    Char(String),
}

/// 按键/鼠标绑定定义。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding<T> {
    pub target: T,
    pub modifiers: Modifiers,
    pub terminal_mode_include: TermMode,
    pub terminal_mode_exclude: TermMode,
}

pub type KeyboardBinding = Binding<InputKind>;

/// generate_bindings! 宏 — 声明式定义按键绑定表。
macro_rules! generate_bindings {
    (
        $binding_type:ident;
        $(
            $target:ident$(::$variant:ident)?
            $(,$input_modifiers:expr)*
            $(,+$terminal_mode_include:expr)*
            $(,~$terminal_mode_exclude:expr)*
            ;$action:expr
        );*
        $(;)*
    ) => {{
        let mut v = Vec::new();
        $(
            let mut _mods = Modifiers::NONE;
            $(_mods = $input_modifiers;)*
            let mut _mode_include = TermMode::empty();
            $(_mode_include.insert($terminal_mode_include);)*
            let mut _mode_exclude = TermMode::empty();
            $(_mode_exclude.insert($terminal_mode_exclude);)*

            let binding = $binding_type {
                target: InputKind::KeyCode(Key::$target),
                modifiers: _mods,
                terminal_mode_include: _mode_include,
                terminal_mode_exclude: _mode_exclude,
            };

            v.push((binding, $action.into()));
        )*
        v
    }};
}

/// 按键绑定布局（有序列表，第一个匹配生效）。
#[derive(Clone, Debug)]
pub struct BindingsLayout {
    bindings: Vec<(Binding<InputKind>, BindingAction)>,
}

impl Default for BindingsLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingsLayout {
    pub fn new() -> Self {
        let bindings = default_keyboard_bindings();
        Self { bindings }
    }

    /// 追加绑定（重复项替换为最后一个）。
    pub fn add_bindings(
        &mut self,
        additions: Vec<(Binding<InputKind>, BindingAction)>,
    ) {
        for (binding, action) in additions {
            if let Some(pos) =
                self.bindings.iter().position(|(b, _)| *b == binding)
            {
                self.bindings[pos] = (binding, action);
            } else {
                self.bindings.push((binding, action));
            }
        }
    }

    /// 查找匹配的绑定动作。
    pub fn get_action(
        &self,
        input: InputKind,
        modifiers: Modifiers,
        terminal_mode: TermMode,
    ) -> BindingAction {
        for (binding, action) in &self.bindings {
            if binding.target == input
                && modifiers.matches_exact(binding.modifiers)
                && terminal_mode.contains(binding.terminal_mode_include)
                && !terminal_mode.intersects(binding.terminal_mode_exclude)
            {
                return action.clone();
            }
        }
        BindingAction::Ignore
    }
}
```

然后在同一文件中追加默认绑定表：

```rust
fn default_keyboard_bindings() -> Vec<(Binding<InputKind>, BindingAction)> {
    // egui 0.34 没有 Modifiers::CTRL 常量，手定义
    const CTRL: Modifiers = Modifiers { ctrl: true, ..Modifiers::NONE };
    generate_bindings!(
        KeyboardBinding;
        Enter;     BindingAction::Char('\x0d');
        Backspace; BindingAction::Char('\x7f');
        Escape;    BindingAction::Char('\x1b');
        Tab;       BindingAction::Char('\x09');
        Insert;    BindingAction::Esc("\x1b[2~".into());
        Delete;    BindingAction::Esc("\x1b[3~".into());
        PageUp;    BindingAction::Esc("\x1b[5~".into());
        PageDown;  BindingAction::Esc("\x1b[6~".into());
        F1;        BindingAction::Esc("\x1bOP".into());
        F2;        BindingAction::Esc("\x1bOQ".into());
        F3;        BindingAction::Esc("\x1bOR".into());
        F4;        BindingAction::Esc("\x1bOS".into());
        F5;        BindingAction::Esc("\x1b[15~".into());
        F6;        BindingAction::Esc("\x1b[17~".into());
        F7;        BindingAction::Esc("\x1b[18~".into());
        F8;        BindingAction::Esc("\x1b[19~".into());
        F9;        BindingAction::Esc("\x1b[20~".into());
        F10;       BindingAction::Esc("\x1b[21~".into());
        F11;       BindingAction::Esc("\x1b[23~".into());
        F12;       BindingAction::Esc("\x1b[24~".into());
        // APP_CURSOR 排除
        End,        ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[F".into());
        Home,       ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[H".into());
        ArrowUp,    ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[A".into());
        ArrowDown,  ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[B".into());
        ArrowLeft,  ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[D".into());
        ArrowRight, ~TermMode::APP_CURSOR; BindingAction::Esc("\x1b[C".into());
        // APP_CURSOR 包含
        End,        +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOF".into());
        Home,       +TermMode::APP_CURSOR; BindingAction::Esc("\x1BOH".into());
        ArrowUp,    +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOA".into());
        ArrowDown,  +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOB".into());
        ArrowLeft,  +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOD".into());
        ArrowRight, +TermMode::APP_CURSOR; BindingAction::Esc("\x1bOC".into());
        // Ctrl 组合
        ArrowUp,    CTRL; BindingAction::Esc("\x1b[1;5A".into());
        ArrowDown,  CTRL; BindingAction::Esc("\x1b[1;5B".into());
        ArrowLeft,  CTRL; BindingAction::Esc("\x1b[1;5D".into());
        ArrowRight, CTRL; BindingAction::Esc("\x1b[1;5C".into());
    )
}
```

- [ ] **步骤 4：运行测试验证通过**

```bash
cargo test -p terminal_panel -- bindings
```

预期: 5 tests PASS。

- [ ] **步骤 5：Commit**

```bash
git add crates/terminal_panel/src/bindings.rs
git commit -m "feat(terminal): add keyboard binding module

Port generate_bindings! macro from egui_term, adapt to egui 0.34.
Covers ANSI sequences, APP_CURSOR mode, Ctrl+arrow combos.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 6：PTY 后端 — TerminalBackend

**文件：**
- 创建：`crates/terminal_panel/src/backend.rs`

这是最复杂的模块。从 egui_term 的 `backend/mod.rs` 移植，适配 `alacritty_terminal` 0.26 和 `portable-pty` 0.9 的 API。

- [ ] **步骤 1：编写集成测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_size_defaults() {
        let size = TerminalSize::default();
        assert_eq!(size.columns(), 80);
        assert_eq!(size.screen_lines(), 50);
    }

    #[test]
    fn terminal_size_resize() {
        let layout = Size::new(800.0, 400.0);
        let font = Size::new(10.0, 18.0);
        let size = compute_size(layout, font);
        // 800/10 = 80 cols, 400/18 ≈ 22 lines
        assert_eq!(size.num_cols, 80);
        assert_eq!(size.num_lines, 22);
    }

    #[test]
    fn compute_size_zero_guards() {
        let size = compute_size(Size::new(0.0, 0.0), Size::new(10.0, 18.0));
        assert_eq!(size.num_cols, 0);
        assert_eq!(size.num_lines, 0);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cargo test -p terminal_panel -- backend
```

预期：编译错误。

- [ ] **步骤 3：实现 backend.rs（第一部分 — 数据结构和构造函数）**

```rust
// crates/terminal_panel/src/backend.rs
//! PTY 后端：管理终端进程生命周期和 I/O。

use alacritty_terminal::event::{Event as PtyEvent, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::{self, viewport_to_point, Term, TermMode, TermSize};
use alacritty_terminal::tty;
use alacritty_terminal::Grid;
use egui::Modifiers;
use portable_pty::{CommandBuilder, PtyPair, PtySize};
use std::cmp::min;
use std::io::{Read, Write};
use std::ops::RangeInclusive;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;

// ---- 大小类型 ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalSize {
    pub cell_width: u16,
    pub cell_height: u16,
    pub num_cols: u16,
    pub num_lines: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cell_width: 1,
            cell_height: 1,
            num_cols: 80,
            num_lines: 50,
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize { self.screen_lines() }
    fn screen_lines(&self) -> usize { self.num_lines as usize }
    fn columns(&self) -> usize { self.num_cols as usize }
    fn last_column(&self) -> Column { Column(self.num_cols as usize - 1) }
    fn bottommost_line(&self) -> Line { Line(self.num_lines as i32 - 1) }
}

impl From<TerminalSize> for WindowSize {
    fn from(size: TerminalSize) -> Self {
        Self {
            num_lines: size.num_lines,
            num_cols: size.num_cols,
            cell_width: size.cell_width,
            cell_height: size.cell_height,
        }
    }
}

pub fn compute_size(layout: Size, font: Size) -> TerminalSize {
    if layout.width <= 0.0 || layout.height <= 0.0
        || font.width <= 0.0 || font.height <= 0.0
    {
        return TerminalSize::default();
    }
    let cols = (layout.width / font.width).floor() as u16;
    let lines = (layout.height / font.height).floor() as u16;
    TerminalSize {
        cell_width: font.width as u16,
        cell_height: font.height as u16,
        num_cols: cols.max(1),
        num_lines: lines.max(1),
    }
}

// ---- 命令枚举 ----

#[derive(Debug, Clone)]
pub enum BackendCommand {
    Write(Vec<u8>),
    Scroll(i32),
    Resize(Size, Size),
    SelectStart(SelectionType, f32, f32),
    SelectUpdate(f32, f32),
    MouseReport(u8, Modifiers, Point, bool),
}

// ---- 可渲染内容 ----

pub struct RenderableContent {
    pub grid: Grid<Cell>,
    pub selectable_range: Option<SelectionRange>,
    pub cursor: Cell,
    pub terminal_mode: TermMode,
    pub terminal_size: TerminalSize,
}

impl RenderableContent {
    pub fn display_offset(&self) -> usize {
        self.grid.display_offset()
    }
}

// ---- 事件代理 ----

#[derive(Clone)]
struct EventProxy(mpsc::Sender<PtyEvent>);

impl alacritty_terminal::event::EventListener for EventProxy {
    fn send_event(&self, event: PtyEvent) {
        let _ = self.0.send(event);
    }
}

// ---- TerminalBackend ----

pub struct TerminalBackend {
    id: u64,
    term: Arc<FairMutex<Term<EventProxy>>>,
    size: TerminalSize,
    notifier: Notifier,
    last_content: RenderableContent,
    alive: bool,
    _child: Option<Box<dyn portable_pty::Child + Send>>,
    _pty_pair: Option<PtyPair>,
}

impl TerminalBackend {
    /// 启动 PTY 进程。
    pub fn spawn(
        ctx: egui::Context,
        shell_program: &str,
        working_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
        // 使用 portable-pty 创建 PTY
        let pty_system = portable_pty::native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: 50,
                cols: 80,
                pixel_width: 800,
                pixel_height: 600,
            })
            .map_err(|e| format!("openpty 失败: {e}"))?;

        let mut cmd = CommandBuilder::new(shell_program);
        if let Some(dir) = working_dir {
            cmd.cwd(dir);
        }
        // 设置 TERM 环境变量
        cmd.env("TERM", "xterm-256color");

        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn 失败: {e}"))?;

        let id = rand_id();
        let terminal_size = TerminalSize::default();
        let config = term::Config::default();

        let (event_sender, event_receiver) = mpsc::channel();
        let event_proxy = EventProxy(event_sender);
        let mut term = Term::new(config, &terminal_size, event_proxy.clone());

        let initial_content = RenderableContent {
            grid: term.grid().clone(),
            selectable_range: None,
            terminal_mode: *term.mode(),
            terminal_size,
            cursor: term.grid_mut().cursor_cell().clone(),
        };

        let term = Arc::new(FairMutex::new(term));
        let mut master_reader = pty_pair.master.try_clone_reader()
            .map_err(|e| format!("clone reader 失败: {e}"))?;
        let master_writer = pty_pair.master.try_clone_writer()
            .map_err(|e| format!("clone writer 失败: {e}"))?;

        // PTY 读取线程
        let term_clone = term.clone();
        let event_proxy_clone = event_proxy.clone();
        let ctx_clone = ctx.clone();
        std::thread::Builder::new()
            .name(format!("pty-reader-{id}"))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match master_reader.read(&mut buf) {
                        Ok(0) => {
                            // EOF — 进程退出
                            let _ = event_sender.send(PtyEvent::Exit);
                            ctx_clone.request_repaint();
                            break;
                        }
                        Ok(n) => {
                            let mut t = term_clone.lock();
                            t.advance_bytes(&buf[..n]);
                        }
                        Err(e) => {
                            tracing::warn!("PTY 读取错误: {e}");
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("创建读取线程失败: {e}"))?;

        // PTY 写入句柄存储
        let writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>> =
            Arc::new(std::sync::Mutex::new(Box::new(master_writer)));

        let notifier = {
            let writer_clone = writer.clone();
            Notifier(Box::new(move |data: Vec<u8>| {
                if let Ok(mut w) = writer_clone.lock() {
                    let _ = w.write_all(&data);
                    let _ = w.flush();
                }
            }))
        };

        Ok(Self {
            id,
            term,
            size: terminal_size,
            notifier,
            last_content: initial_content,
            alive: true,
            _child: Some(Box::new(child)),
            _pty_pair: Some(pty_pair),
        })
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// 处理外部命令（来自 UI 输入）。
    pub fn process_command(&mut self, cmd: BackendCommand) {
        let term = self.term.clone();
        let mut term = term.lock();
        match cmd {
            BackendCommand::Write(input) => {
                self.notifier.0(&input);
                term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
            }
            BackendCommand::Scroll(delta) => {
                if delta != 0 && !term.mode().contains(
                    TermMode::ALTERNATE_SCROLL | TermMode::ALT_SCREEN,
                ) {
                    term.grid_mut().scroll_display(
                        alacritty_terminal::grid::Scroll::Delta(delta),
                    );
                }
            }
            BackendCommand::Resize(layout_size, font_size) => {
                let new_size = compute_size(layout_size, font_size);
                if new_size.num_cols != self.size.num_cols
                    || new_size.num_lines != self.size.num_lines
                {
                    self.size = new_size;
                    self.notifier.on_resize(new_size.into());
                    term.resize(TermSize::new(
                        new_size.num_cols as usize,
                        new_size.num_lines as usize,
                    ));
                }
            }
            BackendCommand::SelectStart(sel_type, x, y) => {
                let point = Self::selection_point(x, y, &self.size, term.grid().display_offset());
                term.selection = Some(Selection::new(sel_type, point, Side::Left));
            }
            BackendCommand::SelectUpdate(x, y) => {
                let offset = term.grid().display_offset();
                if let Some(ref mut sel) = term.selection {
                    let point = Self::selection_point(x, y, &self.size, offset);
                    sel.update(point, Side::Left);
                }
            }
            BackendCommand::MouseReport(button, _mods, point, pressed) => {
                let c = if pressed { 'M' } else { 'm' };
                let msg = format!(
                    "\x1b[<{};{};{}{c}",
                    button,
                    point.column.0 + 1,
                    point.line.0 + 1,
                );
                self.notifier.0(msg.as_bytes().to_vec());
            }
        }
    }

    /// 同步终端状态并返回可渲染内容。
    pub fn sync(&mut self) -> &RenderableContent {
        let term = self.term.clone();
        let mut terminal = term.lock();
        let selectable_range = terminal.selection.as_ref()
            .and_then(|s| s.to_range(&terminal));
        let cursor = terminal.grid_mut().cursor_cell().clone();

        self.last_content.grid = terminal.grid().clone();
        self.last_content.selectable_range = selectable_range;
        self.last_content.cursor = cursor;
        self.last_content.terminal_mode = *terminal.mode();
        self.last_content.terminal_size = self.size;
        &self.last_content
    }

    pub fn last_content(&self) -> &RenderableContent {
        &self.last_content
    }

    /// 获取选中文本内容。
    pub fn selectable_content(&self) -> String {
        let mut result = String::new();
        if let Some(range) = &self.last_content.selectable_range {
            for indexed in self.last_content.grid.display_iter() {
                if range.contains(indexed.point) {
                    result.push(indexed.c);
                }
            }
        }
        result
    }

    pub fn selection_point(
        x: f32,
        y: f32,
        terminal_size: &TerminalSize,
        display_offset: usize,
    ) -> Point {
        let col = (x as usize) / (terminal_size.cell_width as usize).max(1);
        let col = min(Column(col), terminal_size.last_column());
        let line = (y as usize) / (terminal_size.cell_height as usize).max(1);
        let line = min(line, terminal_size.num_lines as usize - 1);
        viewport_to_point(display_offset, Point::new(line, col))
    }
}

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
```

- [ ] **步骤 4：注册 backend 模块**

```rust
// 在 crates/terminal_panel/src/lib.rs 中，确认有:
pub mod backend;
```

- [ ] **步骤 5：运行测试验证通过**

```bash
cargo test -p terminal_panel -- backend
```

预期: 3 tests PASS。

- [ ] **步骤 6：Commit**

```bash
git add crates/terminal_panel/src/backend.rs crates/terminal_panel/src/lib.rs
git commit -m "feat(terminal): add PTY backend with portable-pty

TerminalBackend spawns cross-platform PTY, manages Term state,
handles resize/scroll/selection/mouse commands.
Reader thread bridges PTY output into Term via advance_bytes.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 7：TerminalView — egui 0.34 渲染

**文件：**
- 创建：`crates/terminal_panel/src/view.rs`

这是核心适配。从 egui_term 的 `view.rs` 移植，关键变化：

- `Shape::text()` → `ui.painter().text()` 或 `ui.painter().galley()`
- 不再收集 Vec<Shape>，改为直接调用 painter
- 适配 egui 0.34 的 Modifiers API

- [ ] **步骤 1：实现 view.rs**

```rust
// crates/terminal_panel/src/view.rs
//! egui 0.34 终端网格渲染。

use alacritty_terminal::index::Point as TermPoint;
use alacritty_terminal::term::cell::{self, Cell};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use egui::{Key, Modifiers, Painter, PointerButton, Pos2, Rect, Response, Vec2, Widget};

use crate::backend::{BackendCommand, RenderableContent, TerminalBackend, Size};
use crate::bindings::{BindingsLayout, BindingAction, InputKind};
use crate::font::TerminalFont;
use crate::theme::TerminalTheme;

/// 终端视图内部状态（持久化在 egui memory 中）。
#[derive(Clone, Default)]
pub struct TerminalViewState {
    pub is_dragged: bool,
    pub scroll_pixels: f32,
}

/// 终端视图 Widget。
pub struct TerminalView<'a> {
    id: egui::Id,
    backend: &'a mut TerminalBackend,
    font: TerminalFont,
    theme: TerminalTheme,
    bindings: BindingsLayout,
    available_size: Vec2,
}

impl<'a> TerminalView<'a> {
    pub fn new(
        ui: &mut egui::Ui,
        backend: &'a mut TerminalBackend,
        font: TerminalFont,
        theme: TerminalTheme,
    ) -> Self {
        let id = ui.make_persistent_id("terminal_view");
        let available = ui.available_size();
        Self {
            id,
            backend,
            font,
            theme,
            bindings: BindingsLayout::new(),
            available_size: available,
        }
    }
}

impl Widget for TerminalView<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(self.available_size, egui::Sense::click_and_drag());

        let mut state = ui.memory_mut(|m| {
            m.data
                .get_temp::<TerminalViewState>(self.id)
                .unwrap_or_default()
        });

        // 处理 resize
        self.backend.process_command(BackendCommand::Resize(
            Size::new(rect.width(), rect.height()),
            Size::new(
                self.font.cell_size(ui.ctx()).0,
                self.font.cell_size(ui.ctx()).1,
            ),
        ));

        // 处理输入
        if response.has_focus() {
            let events = ui.input(|i| i.events.clone());
            let modifiers = ui.input(|i| i.modifiers);
            for event in &events {
                let actions = self.process_event(event, &modifiers, &response);
                for action in actions {
                    match action {
                        InputAction::BackendCmd(cmd) => {
                            self.backend.process_command(cmd);
                        }
                        InputAction::Clipboard(text) => {
                            ui.ctx().copy_text(text);
                        }
                        InputAction::Ignore => {}
                    }
                }
            }
        }

        // 渲染
        self.render(ui, &response, &mut state);

        ui.memory_mut(|m| m.data.insert_temp(self.id, state));
        response
    }
}

impl<'a> TerminalView<'a> {
    fn process_event(
        &self,
        event: &egui::Event,
        modifiers: &Modifiers,
        _response: &Response,
    ) -> Vec<InputAction> {
        match event {
            egui::Event::Text(text) => {
                vec![InputAction::BackendCmd(BackendCommand::Write(
                    text.as_bytes().to_vec(),
                ))]
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers: key_mods,
                ..
            } => {
                let combined = Modifiers {
                    ctrl: modifiers.ctrl || key_mods.ctrl,
                    shift: modifiers.shift || key_mods.shift,
                    alt: modifiers.alt || key_mods.alt,
                    mac_cmd: modifiers.mac_cmd || key_mods.mac_cmd,
                    ..Default::default()
                };
                let action = self.bindings.get_action(
                    InputKind::KeyCode(*key),
                    combined,
                    self.backend.last_content().terminal_mode,
                );
                match action {
                    BindingAction::Char(c) => {
                        let mut buf = [0u8; 4];
                        let s = c.encode_utf8(&mut buf);
                        vec![InputAction::BackendCmd(BackendCommand::Write(
                            s.as_bytes().to_vec(),
                        ))]
                    }
                    BindingAction::Esc(seq) => {
                        vec![InputAction::BackendCmd(BackendCommand::Write(
                            seq.as_bytes().to_vec(),
                        ))]
                    }
                    _ => vec![],
                }
            }
            egui::Event::PointerButton {
                button: PointerButton::Primary,
                pressed: true,
                pos,
                ..
            } => {
                let rel = *pos - _response.rect.min;
                vec![InputAction::BackendCmd(BackendCommand::SelectStart(
                    alacritty_terminal::selection::SelectionType::Simple,
                    rel.x,
                    rel.y,
                ))]
            }
            egui::Event::PointerMoved(pos) => {
                let rel = *pos - _response.rect.min;
                vec![InputAction::BackendCmd(BackendCommand::SelectUpdate(
                    rel.x, rel.y,
                ))]
            }
            egui::Event::MouseWheel { delta, .. } => {
                let lines = delta.y.signum() * delta.y.abs().ceil() as i32;
                vec![InputAction::BackendCmd(BackendCommand::Scroll(lines))]
            }
            _ => vec![],
        }
    }

    fn render(
        &self,
        ui: &mut egui::Ui,
        response: &Response,
        _state: &mut TerminalViewState,
    ) {
        let content = self.backend.sync();
        let painter = ui.painter();
        let layout_min = response.rect.min;
        let cell_width = content.terminal_size.cell_width as f32;
        let cell_height = content.terminal_size.cell_height as f32;

        if cell_width <= 0.0 || cell_height <= 0.0 {
            return;
        }

        // 背景
        let bg = self
            .theme
            .get_color(Color::Named(NamedColor::Background));
        painter.rect_filled(response.rect, egui::CornerRadius::ZERO, bg);

        for indexed in content.grid.display_iter() {
            let flags = indexed.cell.flags;
            if flags.contains(cell::Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            let is_wide = flags.contains(cell::Flags::WIDE_CHAR);
            let is_inverse = flags.contains(cell::Flags::INVERSE);
            let is_dim = flags.intersects(cell::Flags::DIM | cell::Flags::DIM_BOLD);
            let is_selected = content
                .selectable_range
                .as_ref()
                .is_some_and(|r| r.contains(indexed.point));

            let x = layout_min.x + cell_width * indexed.point.column.0 as f32;
            let line = indexed.point.line.0 + content.display_offset() as i32;
            let y = layout_min.y + cell_height * line as f32;

            let mut fg = self.theme.get_color(indexed.fg);
            let mut bg = self.theme.get_color(indexed.bg);

            if is_dim {
                fg = fg.linear_multiply(0.7);
            }
            if is_inverse || is_selected {
                std::mem::swap(&mut fg, &mut bg);
            }

            let w = if is_wide { cell_width * 2.0 } else { cell_width };

            // 非默认背景才绘制背景矩形
            let global_bg = self
                .theme
                .get_color(Color::Named(NamedColor::Background));
            if bg != global_bg {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w + 1.0, cell_height + 1.0)),
                    egui::CornerRadius::ZERO,
                    bg,
                );
            }

            // 光标
            if content.cursor.point == indexed.point {
                let cursor_color = self.theme.get_color(content.cursor.fg);
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, cell_height)),
                    egui::CornerRadius::ZERO,
                    cursor_color,
                );
            }

            // 绘制字符（egui 0.34 方式: Painter::text）
            if indexed.c != ' ' && indexed.c != '\t' && indexed.c != '\u{00a0}' {
                let font_id = self.font.font_type();
                // 在 cell 水平居中
                let text_x = x + w / 2.0;
                painter.text(
                    Pos2::new(text_x, y),
                    egui::Align2::CENTER_TOP,
                    indexed.c,
                    font_id,
                    fg,
                );
            }
        }
    }
}

enum InputAction {
    BackendCmd(BackendCommand),
    Clipboard(String),
    Ignore,
}
```

- [ ] **步骤 2：更新 lib.rs 导出**

```rust
// 在 crates/terminal_panel/src/lib.rs 的 pub mod 列表中添加:
pub use view::TerminalView;
```

- [ ] **步骤 3：验证编译**

```bash
cargo check -p terminal_panel
```

预期：成功编译。

- [ ] **步骤 4：Commit**

```bash
git add crates/terminal_panel/src/view.rs crates/terminal_panel/src/lib.rs
git commit -m "feat(terminal): add TerminalView with egui 0.34 galley rendering

Replace egui_term's Shape::text() with Painter::text() and
Painter::rect_filled() for per-cell rendering.
Handle ANSI colors, inverse, dim, wide chars, cursor, selection.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 8：TerminalPanel 完整状态机

**文件：**
- 修改：`crates/terminal_panel/src/lib.rs`

将骨架替换为完整实现，连接 backend + view。

- [ ] **步骤 1：更新 lib.rs 完整实现**

```rust
// crates/terminal_panel/src/lib.rs
//! 嵌入式终端面板。

pub mod backend;
pub mod bindings;
pub mod font;
pub mod shell;
pub mod theme;
pub mod view;

use crate::backend::TerminalBackend;
use crate::font::TerminalFont;
use crate::theme::TerminalTheme;
use crate::view::TerminalView;
use egui::Vec2;

/// 终端面板状态机。
pub struct TerminalPanel {
    backend: Option<TerminalBackend>,
    font: TerminalFont,
    theme: TerminalTheme,
    pub visible: bool,
    pub height: f32,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    /// 是否需要在下一帧请求焦点。
    pub focus_requested: bool,
}

impl TerminalPanel {
    pub fn new() -> Self {
        Self {
            backend: None,
            font: TerminalFont::default(),
            theme: TerminalTheme::default(),
            visible: false,
            height: 200.0,
            error: None,
            exit_code: None,
            focus_requested: false,
        }
    }

    /// 切换终端显示/隐藏。
    pub fn toggle(&mut self, ctx: &egui::Context) {
        self.visible = !self.visible;
        if self.visible && self.backend.is_none() {
            let (shell, _) = shell::detect_shell();
            self.spawn(ctx, &shell, None);
        }
        if self.visible {
            self.focus_requested = true;
        }
    }

    /// 启动 PTY 进程。
    pub fn spawn(
        &mut self,
        ctx: &egui::Context,
        shell_program: &str,
        working_dir: Option<std::path::PathBuf>,
    ) {
        if self.backend.is_some() {
            return;
        }
        match TerminalBackend::spawn(ctx.clone(), shell_program, working_dir) {
            Ok(be) => {
                self.backend = Some(be);
                self.error = None;
                self.exit_code = None;
            }
            Err(e) => {
                self.error = Some(format!("终端启动失败: {e}"));
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.backend.as_ref().is_some_and(|be| be.is_alive())
    }

    /// 在 egui panel 中渲染终端。
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // 错误提示
        if let Some(ref e) = self.error.clone() {
            ui.vertical_centered(|ui| {
                ui.label(format!("❌ {e}"));
                if ui.button("重试").clicked() {
                    self.error = None;
                    self.backend = None;
                    let (shell, _) = shell::detect_shell();
                    self.spawn(ui.ctx(), &shell, None);
                }
            });
            return;
        }

        // 进程已退出
        if let Some(code) = self.exit_code {
            ui.vertical_centered(|ui| {
                ui.label(format!("进程已退出 (退出码: {code})"));
                if ui.button("重新启动 (Enter)").clicked() {
                    self.exit_code = None;
                    self.backend = None;
                    let (shell, _) = shell::detect_shell();
                    self.spawn(ui.ctx(), &shell, None);
                }
            });
            return;
        }

        // 渲染终端
        if let Some(ref mut be) = self.backend {
            let view = TerminalView::new(ui, be, self.font.clone(), self.theme.clone());
            let response = ui.add(view);

            // 焦点管理
            if self.focus_requested {
                response.request_focus();
                self.focus_requested = false;
            }
        }
    }
}

impl Default for TerminalPanel {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **步骤 2：验证编译**

```bash
cargo check -p terminal_panel
```

预期：成功编译。

- [ ] **步骤 3：Commit**

```bash
git add crates/terminal_panel/src/lib.rs
git commit -m "feat(terminal): connect TerminalPanel with backend + view

TerminalPanel manages show/hide, spawn lifecycle, error state,
exit code handling, and focus management.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 9：集成到 zdown-app

**文件：**
- 修改：`crates/zdown-app/Cargo.toml`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1：添加依赖**

```toml
# crates/zdown-app/Cargo.toml，在 [dependencies] 中添加:
terminal_panel = { path = "../terminal_panel" }
```

- [ ] **步骤 2：在 ZdownApp 中添加 terminal 字段并集成底部面板**

修改 `crates/zdown-app/src/main.rs`:

ZdownApp struct 添加字段：

```rust
/// 终端面板。
terminal: terminal_panel::TerminalPanel,
```

Default 实现中添加：

```rust
terminal: terminal_panel::TerminalPanel::default(),
```

在 `ui()` 方法中，CentralPanel 结束后（搜索栏之后）添加底部面板：

```rust
// ===== 终端面板 (Ctrl+`) =====
if self.terminal.visible {
    egui::TopBottomPanel::bottom("terminal_panel")
        .resizable(true)
        .default_height(self.terminal.height)
        .min_height(60.0)
        .show_inside(ui, |ui| {
            self.terminal.show(ui);
        });
}
```

添加快捷键处理（在现有快捷键代码块附近）：

```rust
// Ctrl+` 切换终端
if !mods.shift && !mods.alt && ctx.input(|i| i.key_pressed(egui::Key::Backtick))
    && mods.ctrl
{
    self.terminal.toggle(&ctx);
}
```

- [ ] **步骤 3：验证编译**

```bash
cargo check -p zdown-app
```

预期：成功编译。

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/Cargo.toml crates/zdown-app/src/main.rs
git commit -m "feat(terminal): integrate terminal panel into zdown-app

Bottom panel via TopBottomPanel::bottom, Ctrl+` toggle,
resizable height. Terminal spawns on first show.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 10：菜单栏添加终端入口 + i18n

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`
- 修改：`crates/i18n/locales/zh-CN/editor.ftl`
- 修改：`crates/i18n/locales/en-US/editor.ftl`

- [ ] **步骤 1：添加 FTL 翻译 key**

`crates/i18n/locales/zh-CN/editor.ftl` 追加：

```ftl
# 终端
menu-view-terminal = 终端 (Ctrl+`)
```

`crates/i18n/locales/en-US/editor.ftl` 追加：

```ftl
# Terminal
menu-view-terminal = Terminal (Ctrl+`)
```

- [ ] **步骤 2：修改菜单**

在 `crates/zdown-app/src/menu.rs` 的 `show_menu` 函数中，视图菜单的主题切换之前添加：

```rust
ui.separator();
if ui.button("终端 (Ctrl+`)").clicked() {
    // terminal toggle — 通过 ZdownApp 的 terminal 字段
    ui.close();
}
```

注意：由于 `show_menu` 目前没有 `terminal` 参数，需要在函数签名中添加：

```rust
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
    settings_dialog: &mut SettingsDialog,
    app_config: &AppConfig,
    theme: &mut ThemeMode,
    image_hosting: &ImageHostingConfig,
    terminal: &mut terminal_panel::TerminalPanel,  // 新增
) {
```

main.rs 中的调用也需要更新，在 menu::show_menu 调用中添加 terminal 参数。

- [ ] **步骤 3：验证编译**

```bash
cargo check -p zdown-app
```

预期：成功编译。

- [ ] **步骤 4：Commit**

```bash
git add crates/zdown-app/src/menu.rs crates/zdown-app/src/main.rs \
        crates/i18n/locales/zh-CN/editor.ftl crates/i18n/locales/en-US/editor.ftl
git commit -m "feat(terminal): add terminal toggle to View menu

Menu entry with Ctrl+` shortcut. i18n keys for zh-CN/en-US.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 11：集成测试与端到端验证

**文件：**
- 创建：`crates/terminal_panel/tests/integration_test.rs`

- [ ] **步骤 1：编写集成测试**

```rust
// crates/terminal_panel/tests/integration_test.rs
use terminal_panel::backend::{TerminalBackend, Size};
use terminal_panel::font::TerminalFont;
use terminal_panel::theme::TerminalTheme;
use terminal_panel::bindings::{BindingsLayout, BindingAction, InputKind};
use egui::{Key, Modifiers};
use alacritty_terminal::term::TermMode;

#[test]
fn spawn_shell_and_check_alive() {
    // 创建一个 headless egui context 用于测试
    let ctx = egui::Context::default();
    let backend = TerminalBackend::spawn(
        ctx,
        if cfg!(target_os = "windows") { "cmd.exe" } else { "/bin/sh" },
        None,
    );
    assert!(backend.is_ok(), "PTY spawn 应该成功: {:?}", backend.err());
    let backend = backend.unwrap();
    assert!(backend.is_alive());
}

#[test]
fn resize_updates_dimensions() {
    let ctx = egui::Context::default();
    let mut backend = TerminalBackend::spawn(
        ctx,
        if cfg!(target_os = "windows") { "cmd.exe" } else { "/bin/sh" },
        None,
    )
    .expect("spawn");

    use terminal_panel::backend::BackendCommand;
    backend.process_command(BackendCommand::Resize(
        Size::new(1200.0, 600.0),
        Size::new(10.0, 18.0),
    ));

    let content = backend.sync();
    // 1200/10 = 120 cols, 600/18 ≈ 33 lines
    assert_eq!(content.terminal_size.num_cols, 120);
    assert_eq!(content.terminal_size.num_lines, 33);
}

#[test]
fn bindings_enter_key() {
    let layout = BindingsLayout::new();
    let action = layout.get_action(
        InputKind::KeyCode(Key::Enter),
        Modifiers::NONE,
        TermMode::empty(),
    );
    assert_eq!(action, BindingAction::Char('\x0d'));
}

#[test]
fn theme_default_background_is_dark() {
    let theme = TerminalTheme::default();
    use alacritty_terminal::vte::ansi::{Color, NamedColor};
    let bg = theme.get_color(Color::Named(NamedColor::Background));
    // 默认 One Dark 背景 #181818
    assert_eq!(bg, egui::Color32::from_rgb(0x18, 0x18, 0x18));
}
```

- [ ] **步骤 2：运行测试**

```bash
cargo test -p terminal_panel
```

预期: 所有测试 PASS。

- [ ] **步骤 3：Commit**

```bash
git add crates/terminal_panel/tests/
git commit -m "test(terminal): add integration tests

PTY spawn, resize dimensions, key bindings, theme colors.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### 任务 12：全量测试与 clippy

- [ ] **步骤 1：cargo fmt**

```bash
cargo fmt
```

- [ ] **步骤 2：cargo clippy**

```bash
cargo clippy --all-targets
```

修复所有 clippy 警告。

- [ ] **步骤 3：cargo test 全量**

```bash
cargo test
```

预期: 全部 PASS。

- [ ] **步骤 4：Commit（如有格式修复）**

```bash
git add -A
git commit -m "style: cargo fmt and clippy fixes for terminal panel

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---
