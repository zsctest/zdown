//! zdown-app：egui 应用入口（阶段 1）。

mod editor_state;
mod menu;
mod source_view;

use editor_state::EditorState;
use eframe::egui;
use menu::ConfirmDialog;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("zdown 启动（阶段 1）");

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
}

impl eframe::App for ZdownApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        menu::show_menu(ui, &mut self.state, &mut self.confirm);
        menu::handle_shortcuts(&ctx, &mut self.state, &mut self.confirm);
        menu::show_confirm_dialog(&ctx, &mut self.state, &mut self.confirm);
        source_view::show_source_view(ui, &mut self.state);

        if self.state.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
