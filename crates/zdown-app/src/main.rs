//! zdown-app：egui 应用入口（阶段 2）。

mod editor_state;
mod hybrid_view;
mod menu;
mod preview_view;
mod source_view;
mod view_mode;

use editor_state::EditorState;
use eframe::egui;
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

struct ZdownApp {
    state: EditorState,
    confirm: ConfirmDialog,
    view_mode: ViewMode,
    /// 缓存上次窗口标题，避免每帧 send_viewport_cmd。
    last_title: String,
    /// 缓存 SourceHighlighter 避免每帧重建。
    highlighter: Option<markdown_renderer::SourceHighlighter>,
    /// 渲染缓存（LRU 10 条）。
    render_cache: markdown_renderer::RenderCache,
}

impl Default for ZdownApp {
    fn default() -> Self {
        Self {
            state: EditorState::default(),
            confirm: ConfirmDialog::default(),
            view_mode: ViewMode::default(),
            last_title: String::new(),
            highlighter: markdown_renderer::SourceHighlighter::new().ok(),
            render_cache: markdown_renderer::RenderCache::new(),
        }
    }
}

impl eframe::App for ZdownApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        menu::show_menu(ui, &mut self.state, &mut self.confirm, &mut self.view_mode);
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

        let highlighter = self.highlighter.as_ref();

        // 根据视图模式渲染
        match self.view_mode {
            ViewMode::Source => source_view::show_source_view(ui, &mut self.state, highlighter),
            ViewMode::Preview => {
                preview_view::show_preview_view(ui, &mut self.state, &mut self.render_cache);
            }
            ViewMode::Hybrid => {
                hybrid_view::show_hybrid_view(
                    ui,
                    &mut self.state,
                    highlighter,
                    &mut self.render_cache,
                );
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
