//! PTY 后端：管理终端进程生命周期和 I/O。
//!
//! 基于 portable-pty (跨平台 PTY) + alacritty_terminal (VTE 解析)。
//!
//! 使用 `vte::ansi::Processor` 将 PTY 输出字节送入 `Term` 进行 VTE 解析，
//! 因为 alacritty_terminal 0.25 的 `Term` 没有公开的字节输入方法。

use alacritty_terminal::Grid;
use alacritty_terminal::event::{Event as PtyEvent, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::{self, Term, TermMode, viewport_to_point};
use alacritty_terminal::vte::ansi::Processor as VteProcessor;
use egui::Modifiers;
use portable_pty::{CommandBuilder, PtySize};
use std::cmp::min;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc;

// ---- 大小类型 ----

/// 布局/字体尺寸（像素）。
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

impl From<egui::Vec2> for Size {
    fn from(v: egui::Vec2) -> Self {
        Self {
            width: v.x,
            height: v.y,
        }
    }
}

/// 终端逻辑尺寸（行列数 + 单元格大小）。
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
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }
    fn screen_lines(&self) -> usize {
        self.num_lines as usize
    }
    fn columns(&self) -> usize {
        self.num_cols as usize
    }
    fn last_column(&self) -> Column {
        Column((self.num_cols as usize).saturating_sub(1))
    }
    fn bottommost_line(&self) -> Line {
        Line(self.num_lines as i32 - 1)
    }
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

/// 根据布局区域和字体大小计算行列数。
pub fn compute_size(layout: Size, font: Size) -> TerminalSize {
    if layout.width <= 0.0 || layout.height <= 0.0 {
        return TerminalSize::default();
    }
    let font_w = if font.width > 0.0 { font.width } else { 1.0 };
    let font_h = if font.height > 0.0 { font.height } else { 1.0 };
    let cols = (layout.width / font_w).floor() as u16;
    let lines = (layout.height / font_h).floor() as u16;
    TerminalSize {
        cell_width: font_w as u16,
        cell_height: font_h as u16,
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
    ProcessLink(Point),
}

// ---- 可渲染内容 ----

pub struct RenderableContent {
    pub grid: Grid<Cell>,
    pub selectable_range: Option<SelectionRange>,
    /// 光标所在单元格的内容。
    pub cursor: Cell,
    /// 光标在网格中的逻辑位置。
    pub cursor_point: Point,
    pub terminal_mode: TermMode,
    pub terminal_size: TerminalSize,
    pub hovered_hyperlink: Option<(Point, Point)>,
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
    #[allow(dead_code)]
    id: u64,
    term: Arc<FairMutex<Term<EventProxy>>>,
    size: TerminalSize,
    writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    last_content: RenderableContent,
    alive: bool,
    /// 持有 PtyPair 以保持 PTY 进程存活
    _pty_pair: Option<portable_pty::PtyPair>,
    #[allow(dead_code)]
    event_sender: mpsc::Sender<PtyEvent>,
}

impl TerminalBackend {
    /// 启动 PTY 进程。
    pub fn spawn(
        ctx: egui::Context,
        shell_program: &str,
        working_dir: Option<std::path::PathBuf>,
    ) -> Result<Self, String> {
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
        cmd.env("TERM", "xterm-256color");

        let _child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn 失败: {e}"))?;

        let id = rand_id();
        let terminal_size = TerminalSize::default();
        let config = term::Config::default();

        let (event_sender, event_receiver) = mpsc::channel();
        let event_proxy = EventProxy(event_sender.clone());
        let mut term = Term::new(config, &terminal_size, event_proxy.clone());

        let initial_content = RenderableContent {
            grid: term.grid().clone(),
            selectable_range: None,
            terminal_mode: *term.mode(),
            terminal_size,
            cursor: term.grid_mut().cursor_cell().clone(),
            cursor_point: term.grid().cursor.point,
            hovered_hyperlink: None,
        };

        let term = Arc::new(FairMutex::new(term));

        // PTY I/O: clone reader for background thread, take writer for main thread
        let mut master_reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("clone reader 失败: {e}"))?;
        let master_writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| format!("take_writer 失败: {e}"))?;

        let writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>> =
            Arc::new(std::sync::Mutex::new(master_writer));

        // PTY 读取线程：将 PTY 输出字节送入 Term 进行 VTE 解析
        let term_clone = term.clone();
        let event_sender_clone = event_sender.clone();
        let ctx_clone = ctx.clone();
        std::thread::Builder::new()
            .name(format!("pty-reader-{id}"))
            .spawn(move || {
                let mut buf = [0u8; 4096];
                let mut processor: VteProcessor = VteProcessor::new();
                loop {
                    match master_reader.read(&mut buf) {
                        Ok(0) => {
                            let _ = event_sender_clone.send(PtyEvent::Exit);
                            ctx_clone.request_repaint();
                            break;
                        }
                        Ok(n) => {
                            let mut t = term_clone.lock();
                            processor.advance(&mut *t, &buf[..n]);
                        }
                        Err(e) => {
                            tracing::warn!("PTY 读取错误: {e}");
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("创建读取线程失败: {e}"))?;

        // 事件转发线程：接收 Term 内部事件并触发重绘
        let ctx_event = ctx.clone();
        std::thread::Builder::new()
            .name(format!("pty-event-{id}"))
            .spawn(move || {
                while let Ok(event) = event_receiver.recv() {
                    ctx_event.request_repaint();
                    if let PtyEvent::Exit = event {
                        break;
                    }
                }
            })
            .map_err(|e| format!("创建事件线程失败: {e}"))?;

        Ok(Self {
            id,
            term,
            size: terminal_size,
            writer,
            last_content: initial_content,
            alive: true,
            _pty_pair: Some(pty_pair),
            event_sender,
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
                self.write_to_pty(&input);
                term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
            }
            BackendCommand::Scroll(delta) => {
                if delta != 0
                    && !term
                        .mode()
                        .contains(TermMode::ALTERNATE_SCROLL | TermMode::ALT_SCREEN)
                {
                    term.grid_mut()
                        .scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
                }
            }
            BackendCommand::Resize(layout_size, font_size) => {
                let new_size = compute_size(layout_size, font_size);
                if new_size.num_cols != self.size.num_cols
                    || new_size.num_lines != self.size.num_lines
                {
                    self.size = new_size;
                    // TerminalSize 实现了 Dimensions，可直接传给 resize
                    term.resize(new_size);
                    // 通知 PTY 尺寸变更
                    self.resize_pty(new_size);
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
                self.write_to_pty(msg.as_bytes());
            }
            BackendCommand::ProcessLink(_point) => {
                // 简化实现：暂不支持 URL 检测和打开
                tracing::debug!("ProcessLink 暂未实现");
            }
        }
    }

    /// 同步终端状态并返回可渲染内容。
    pub fn sync(&mut self) -> &RenderableContent {
        let term = self.term.clone();
        let mut terminal = term.lock();
        let selectable_range = terminal
            .selection
            .as_ref()
            .and_then(|s| s.to_range(&terminal));
        let cursor = terminal.grid_mut().cursor_cell().clone();
        let cursor_point = terminal.grid().cursor.point;

        self.last_content.grid = terminal.grid().clone();
        self.last_content.selectable_range = selectable_range;
        self.last_content.cursor = cursor;
        self.last_content.cursor_point = cursor_point;
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

    pub fn selection_point(x: f32, y: f32, size: &TerminalSize, display_offset: usize) -> Point {
        let cw = (size.cell_width as usize).max(1);
        let ch = (size.cell_height as usize).max(1);
        let col = min(Column((x as usize) / cw), size.last_column());
        let line = (y as usize) / ch;
        let line = min(line, size.num_lines as usize - 1);
        viewport_to_point(display_offset, Point::new(line, col))
    }

    fn write_to_pty(&self, data: &[u8]) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(data);
            let _ = w.flush();
        }
    }

    fn resize_pty(&self, size: TerminalSize) {
        if let Ok(mut w) = self.writer.lock() {
            // 通过 ANSI 转义序列设置终端大小
            let _ = w.write_all(format!("\x1b[8;{};{}t", size.num_lines, size.num_cols).as_bytes());
        }
    }
}

fn rand_id() -> u64 {
    #[allow(clippy::unwrap_used)]
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

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
    fn compute_size_standard() {
        let layout = Size::new(800.0, 400.0);
        let font = Size::new(10.0, 18.0);
        let size = compute_size(layout, font);
        assert_eq!(size.num_cols, 80);
        assert_eq!(size.num_lines, 22);
    }

    #[test]
    fn compute_size_zero_guards() {
        let size = compute_size(Size::new(0.0, 0.0), Size::new(10.0, 18.0));
        assert_eq!(size.num_cols, 80); // returns default
        assert_eq!(size.num_lines, 50); // returns default
    }

    #[test]
    fn size_from_vec2() {
        let v = egui::Vec2::new(100.0, 200.0);
        let s: Size = v.into();
        assert_eq!(s.width, 100.0);
        assert_eq!(s.height, 200.0);
    }
}
