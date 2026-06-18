# zdown-app 视图模式切换实现计划

> **面向 AI 代理的工作者：** 必需子智能体：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。

**目标：** 实现 zdown-app 源码/预览/hybrid 三模式切换 + Ctrl+1/2/3 快捷键 + 实时预览基础。

**架构：** `zdown-app` 加 `ViewMode` enum（Source/Preview/Hybrid），`ZdownApp` 持 `view_mode` 字段。源码模式复用阶段 1 `source_view`，预览模式调用 `markdown_renderer::render`，hybrid 模式留 Plan 4。Ctrl+1/2/3 切换模式，菜单加"视图"项。

**技术栈：** Rust 2024 edition、egui 0.34、eframe 0.34、markdown_renderer（path）。

**前置任务：** Plan 1（markdown_renderer AST 渲染）完成。

---

## 文件结构

- 创建：`crates/zdown-app/src/view_mode.rs` — `ViewMode` enum + 切换逻辑
- 创建：`crates/zdown-app/src/preview_view.rs` — 预览模式视图
- 修改：`crates/zdown-app/src/main.rs` — 集成视图模式
- 修改：`crates/zdown-app/src/menu.rs` — 加视图菜单 + Ctrl+1/2/3

**关键设计决策：**

- **ViewMode enum**：`Source` / `Preview` / `Hybrid`，默认 `Source`
- **切换不丢失光标**：切换时保留 EditorState，仅改渲染路径
- **实时预览**：Preview 模式下，每次 `update` 从 EditorState 当前文本重新 parse + render
- **hybrid 模式**：本 plan 仅占位（切换到 Hybrid 暂时显示 Preview），完整实现留 Plan 4
- **快捷键**：Ctrl+1 Source / Ctrl+2 Preview / Ctrl+3 Hybrid

---

## 任务 1：ViewMode enum + 预览视图

**文件：**
- 创建：`crates/zdown-app/src/view_mode.rs`
- 创建：`crates/zdown-app/src/preview_view.rs`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 1.1：创建 view_mode.rs**

创建 `crates/zdown-app/src/view_mode.rs`：

```rust
//! 视图模式：源码 / 预览 / hybrid。

/// 视图模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// 源码编辑模式。
    #[default]
    Source,
    /// 预览模式（只读渲染）。
    Preview,
    /// Hybrid 模式（光标处源码，其余渲染）。
    Hybrid,
}

impl ViewMode {
    /// 中文名（菜单显示）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "源码",
            Self::Preview => "预览",
            Self::Hybrid => "Hybrid",
        }
    }
}
```

> **注意：** 不在 view_mode.rs 定义 `from_key`，直接在 main.rs 用 `ctx.input(|i| i.key_pressed(egui::Key::Num1))` 逐键判断，避免 `from_key` 成死代码。

- [ ] **步骤 1.2：创建 preview_view.rs**

创建 `crates/zdown-app/src/preview_view.rs`：

```rust
//! 预览模式视图：AST → egui 渲染。

use eframe::egui;

use crate::editor_state::EditorState;

/// 渲染预览视图。
pub fn show_preview_view(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let doc = state.current_doc();
        markdown_renderer::render(ui, &doc);
    });
}
```

- [ ] **步骤 1.3：修改 main.rs 集成视图模式**

修改 `crates/zdown-app/src/main.rs`。注意：本任务 `show_menu` 调用先用阶段 1 的 3 参数签名，任务 2 再改 4 参数：

```rust
//! zdown-app：egui 应用入口（阶段 2）。

mod editor_state;
mod menu;
mod preview_view;
mod source_view;
mod view_mode;

use eframe::egui;
use editor_state::EditorState;
use menu::ConfirmDialog;
use view_mode::ViewMode;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 2）");

    if std::env::var_os("ZDOWN_SMOKE").is_some() {
        tracing::info!("ZDOWN_SMOKE 已设置，跳过 GUI 启动");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("zdown"),
        ..Default::default()
    };

    eframe::run_native(
        "zdown",
        options,
        Box::new(|_cc| Ok(Box::new(ZdownApp::default()))),
    )
}

#[derive(Default)]
struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    /// 缓存上次窗口标题，避免每帧 send_viewport_cmd。
    last_title: String,
}

impl eframe::App for ZdownApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // 注意：show_menu 调用先用 3 参数（阶段 1 签名），任务 2 改 4 参数
        menu::show_menu(ui, &mut self.state, &mut self.confirm);
        menu::handle_shortcuts(&ctx, &mut self.state, &mut self.confirm);

        // 视图模式快捷键 Ctrl+1/2/3
        let mods = ctx.input(|i| i.modifiers);
        if mods.ctrl && !mods.shift {
            if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
                self.view_mode = ViewMode::Source;
                tracing::info!("切换到源码模式");
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
                self.view_mode = ViewMode::Preview;
                tracing::info!("切换到预览模式");
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
                self.view_mode = ViewMode::Hybrid;
                tracing::info!("切换到 Hybrid 模式");
            }
        }

        menu::show_confirm_dialog(&ctx, &mut self.state, &mut self.confirm);

        // 根据视图模式渲染
        match self.view_mode {
            ViewMode::Source => source_view::show_source_view(ui, &mut self.state),
            ViewMode::Preview => preview_view::show_preview_view(ui, &mut self.state),
            ViewMode::Hybrid => {
                // 阶段 2 占位：Hybrid 暂用 Preview，Plan 4 完整实现
                preview_view::show_preview_view(ui, &mut self.state);
            }
        }

        if self.state.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // 更新窗口标题（只在变化时发送，避免每帧触发窗口管理器）
        let title = format!("{} [{}]", self.state.title(), self.view_mode.label());
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}
```

- [ ] **步骤 1.4：编译验证 + smoke**

运行：`cargo build -p zdown-app`
预期：编译通过。

运行：`ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：打印 info 日志后退出。

- [ ] **步骤 1.5：Commit**

```bash
git add crates/zdown-app/src/view_mode.rs crates/zdown-app/src/preview_view.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): ViewMode enum + 预览视图

ViewMode: Source/Preview/Hybrid，默认 Source。
preview_view 调用 markdown_renderer::render 渲染 AST。
Ctrl+1/2/3 切换模式。Hybrid 暂占位用 Preview。"
```

---

## 任务 2：视图菜单

**文件：**
- 修改：`crates/zdown-app/src/menu.rs`
- 修改：`crates/zdown-app/src/main.rs`

- [ ] **步骤 2.1：menu.rs 加视图菜单（完整 show_menu 函数）**

修改 `crates/zdown-app/src/menu.rs`，替换整个 `show_menu` 函数。在文件顶部加 `use crate::view_mode::ViewMode;`：

```rust
use crate::view_mode::ViewMode;

#[allow(deprecated)]
pub fn show_menu(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    confirm: &mut ConfirmDialog,
    view_mode: &mut ViewMode,
) {
    egui::TopBottomPanel::top("menu").show_inside(ui, |ui| {
        #[allow(deprecated)]
        egui::menu::bar(ui, |ui| {
            // 文件菜单
            ui.menu_button("文件", |ui| {
                if ui.button("新建 (Ctrl+N)").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::New);
                    } else {
                        state.new_file();
                    }
                }
                if ui.button("打开... (Ctrl+O)").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::Open);
                    } else {
                        trigger_open(state);
                    }
                }
                if ui.button("保存 (Ctrl+S)").clicked() {
                    if state.current_path.is_none() {
                        trigger_save_as(state);
                    } else {
                        let _ = state.save();
                    }
                }
                if ui.button("另存为... (Ctrl+Shift+S)").clicked() {
                    trigger_save_as(state);
                }

                ui.separator();

                ui.menu_button("最近文件", |ui| {
                    if state.recent.list().is_empty() {
                        ui.label("(无)");
                    } else {
                        for path in state.recent.list().to_vec() {
                            if ui.button(path.display().to_string()).clicked() {
                                let _ = state.open(&path);
                                ui.close();
                            }
                        }
                    }
                });

                ui.separator();

                if ui.button("退出").clicked() {
                    if state.is_dirty() {
                        confirm.pending = Some(PendingAction::Quit);
                    } else {
                        state.quit();
                    }
                }
            });

            // 编辑菜单
            ui.menu_button("编辑", |ui| {
                if ui.button("撤销 (Ctrl+Z)").clicked() {
                    let _ = state.undo();
                }
                if ui.button("重做 (Ctrl+Y)").clicked() {
                    let _ = state.redo();
                }
            });

            // 视图菜单
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
        });
    });
}
```

- [ ] **步骤 2.2：main.rs 更新 show_menu 调用为 4 参数**

修改 `crates/zdown-app/src/main.rs` 的 `ZdownApp::ui`，把任务 1.3 中的 `menu::show_menu(ui, &mut self.state, &mut self.confirm);` 改为：

```rust
menu::show_menu(ui, &mut self.state, &mut self.confirm, &mut self.view_mode);
```

- [ ] **步骤 2.3：编译验证 + smoke**

运行：`cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app`
预期：编译通过 + smoke 不 panic。

运行：`cargo clippy -p zdown-app --all-targets -- -D warnings`
预期：无警告。

- [ ] **步骤 2.4：Commit**

```bash
git add crates/zdown-app/src/menu.rs crates/zdown-app/src/main.rs
git commit -m "feat(zdown-app): 视图菜单（源码/预览/Hybrid）

菜单加'视图'项，点击切换 ViewMode。
show_menu 签名加 view_mode 参数。"
```

---

## 任务 3：实时预览验证

**文件：** 无（验证任务）

- [ ] **步骤 3.1：验证实时预览 + 模式切换不丢失光标**

任务 1.3 的 main.rs 已实现：
- 视图模式快捷键 Ctrl+1/2/3 + tracing 日志（任务 1.3 行 154-166）
- 标题栏含模式名 + last_title 缓存（任务 1.3 行 184-189）
- preview_view 每帧重新 parse + render（任务 1.2 的 `state.current_doc()`）

本任务只需验证，无需改代码。运行：

```bash
cargo build -p zdown-app && ZDOWN_SMOKE=1 cargo run -p zdown-app && cargo clippy -p zdown-app --all-targets -- -D warnings
```

预期：全部通过。

- [ ] **步骤 3.2：Commit（仅验证，无代码改动则跳过 commit）**

若 clippy/fmt 有修复：

```bash
git add -A
git commit -m "chore: Plan 2 实时预览验证通过"
```

---

## 任务 4：全量验证

- [ ] **步骤 4.1：fmt + clippy + test + build + smoke**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
ZDOWN_SMOKE=1 cargo run -p zdown-app
```

预期：全部通过。

- [ ] **步骤 4.2：本地手动验证**

- 启动 `cargo run -p zdown-app`
- Ctrl+2 切换到预览，应显示渲染后的 Markdown
- Ctrl+1 切换回源码，光标位置保留
- 编辑后 Ctrl+2，预览应反映编辑内容
- 标题栏显示 `[源码]` / `[预览]` / `[Hybrid]`

- [ ] **步骤 4.3：Commit（如有修复）**

```bash
git add -A
git commit -m "chore: Plan 2 验证通过"
```

---

## 自检

**1. 规格覆盖度：**

- ROADMAP 阶段 2 zdown-app 交付物：
  - 视图模式切换（Ctrl+1/2/3）→ 任务 1/2 ✓
  - hybrid 模式 → 占位，Plan 4 完整实现
  - 实时预览 → 任务 3 ✓
- 验收标准 2（三种模式切换不丢失光标位置）→ 任务 3 ✓
- 验收标准 3（hybrid 模式编辑流畅）→ Plan 4

**2. 占位符扫描：**

- Hybrid 模式占位是设计决策（Plan 4 实现），非计划缺陷
- 每个步骤含完整代码

**3. 类型一致性：**

- `ViewMode` enum 跨任务一致
- `show_menu` 签名加 `view_mode` 参数，跨任务一致
- `show_preview_view(ui, state)` 签名一致

---

## 执行交接

本计划已保存到 `docs/superpowers/plans/2026-06-18-stage2-view-mode-switch.md`。

执行者注意：完成后继续 Plan 3（高亮 + 增量编辑）、Plan 4（hybrid + 渲染缓存）。
