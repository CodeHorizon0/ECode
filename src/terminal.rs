use std::env;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, Context, FontId, Id, PointerButton, Pos2, Rect, Sense, Ui};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

const SCROLLBACK: usize = 5000;
const CELL_HEIGHT: f32 = 17.0;
const FONT_SIZE: f32 = 13.0;
const TERMINAL_BACKGROUND: Color32 = Color32::from_rgb(12, 14, 18);
const TERMINAL_FOREGROUND: Color32 = Color32::from_rgb(210, 214, 224);
const CURSOR_COLOR: Color32 = Color32::from_rgb(196, 213, 255);

struct TerminalSession {
    parser: vt100::Parser,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    output_rx: Receiver<Vec<u8>>,
    resize_rows: u16,
    resize_cols: u16,
    last_resize: Instant,
}

impl TerminalSession {
    fn new(cwd: &Path, rows: u16, cols: u16) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Failed to create PTY: {error}"))?;

        let mut command = shell_command()?;
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "ECode");
        command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Failed to start terminal shell: {error}"))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Failed to open PTY reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Failed to open PTY writer: {error}"))?;

        let (output_tx, output_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => {
                        if output_tx.send(buffer[..size].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            parser: vt100::Parser::new(rows, cols, SCROLLBACK),
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
            output_rx,
            resize_rows: rows,
            resize_cols: cols,
            last_resize: Instant::now(),
        })
    }

    fn poll_output(&mut self) -> bool {
        let mut changed = false;
        while let Ok(bytes) = self.output_rx.try_recv() {
            self.parser.process(&bytes);
            changed = true;
        }
        changed
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }

        self.parser.screen_mut().set_scrollback(0);

        let mut writer = self
            .writer
            .lock()
            .map_err(|_| String::from("Terminal writer lock is poisoned"))?;
        writer
            .write_all(bytes)
            .and_then(|_| writer.flush())
            .map_err(|error| format!("Failed to write to terminal: {error}"))
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        if rows == 0 || cols == 0 {
            return;
        }
        if rows == self.resize_rows && cols == self.resize_cols {
            return;
        }
        if self.last_resize.elapsed() < Duration::from_millis(30) {
            return;
        }

        let resized = self
            .master
            .lock()
            .map(|master| {
                master.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .is_ok()
            })
            .unwrap_or(false);

        if resized {
            self.parser.screen_mut().set_size(rows, cols);
            self.resize_rows = rows;
            self.resize_cols = cols;
            self.last_resize = Instant::now();
        }
    }

    fn is_alive(&self) -> bool {
        self.child
            .lock()
            .map(|mut child| child.try_wait().ok().flatten().is_none())
            .unwrap_or(false)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

struct TerminalInstance {
    id: u64,
    name: String,
    cwd: PathBuf,
    session: TerminalSession,
}

pub struct TerminalManager {
    terminals: Vec<TerminalInstance>,
    active: usize,
    next_id: u64,
    visible: bool,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self {
            terminals: Vec::new(),
            active: 0,
            next_id: 1,
            visible: false,
        }
    }
}

impl TerminalManager {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn ensure_terminal(&mut self, cwd: &Path, ctx: &Context) -> Result<(), String> {
        if !self.terminals.is_empty() {
            return Ok(());
        }
        self.new_terminal(cwd, ctx)
    }

    pub fn new_terminal(&mut self, cwd: &Path, ctx: &Context) -> Result<(), String> {
        let rect = ctx.input(|input| input.content_rect());
        let (rows, cols) = terminal_size(rect.width(), rect.height());
        let session = TerminalSession::new(cwd, rows, cols)?;
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.terminals.push(TerminalInstance {
            id,
            name: shell_display_name(),
            cwd: cwd.to_path_buf(),
            session,
        });
        self.active = self.terminals.len() - 1;
        self.visible = true;
        Ok(())
    }

    pub fn close_active(&mut self) {
        if self.terminals.is_empty() {
            return;
        }
        self.terminals.remove(self.active);
        if self.terminals.is_empty() {
            self.active = 0;
            return;
        }
        self.active = self.active.min(self.terminals.len() - 1);
    }

    pub fn is_focused(&self, ctx: &Context) -> bool {
        if !self.visible {
            return false;
        }
        ctx.memory(|memory| memory.has_focus(terminal_id(self.active_id())))
    }

    fn active_id(&self) -> u64 {
        self.terminals.get(self.active).map(|terminal| terminal.id).unwrap_or(0)
    }

    pub fn ui(&mut self, ui: &mut Ui, ctx: &Context, cwd: &Path) -> Option<String> {
        if !self.visible {
            return None;
        }

        if self.terminals.is_empty() {
            if let Err(error) = self.ensure_terminal(cwd, ctx) {
                return Some(error);
            }
        }

        let mut error = None;
        let mut new_terminal = false;
        let mut close_terminal = false;
        let mut selected = None;

        egui::Panel::bottom("terminal_panel")
            .resizable(true)
            .default_size(300.0)
            .min_size(150.0)
            .max_size(ctx.input(|input| input.content_rect().height()) * 0.65)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.horizontal(|ui| {
                    ui.strong("TERMINAL");
                    ui.separator();

                    for (index, terminal) in self.terminals.iter().enumerate() {
                        let active = index == self.active;
                        if ui
                            .selectable_label(active, format!("{}  {}", terminal.name, index + 1))
                            .clicked()
                        {
                            selected = Some(index);
                        }
                    }

                    ui.separator();
                    if ui.small_button("+").clicked() {
                        new_terminal = true;
                    }
                    if ui.small_button("×").clicked() {
                        close_terminal = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(terminal) = self.terminals.get(self.active) {
                            ui.label(terminal.cwd.display().to_string());
                        }
                    });
                });

                ui.separator();

                if let Some(index) = selected {
                    if index < self.terminals.len() {
                        self.active = index;
                    }
                }

                if new_terminal {
                    if let Err(message) = self.new_terminal(cwd, ctx) {
                        error = Some(message);
                    }
                }

                if close_terminal {
                    self.close_active();
                }

                if let Some(terminal) = self.terminals.get_mut(self.active) {
                    terminal.session.poll_output();
                    if let Err(message) = render_session(ui, ctx, terminal) {
                        error = Some(message);
                    }
                }
            });

        error
    }
}

fn render_session(
    ui: &mut Ui,
    ctx: &Context,
    terminal: &mut TerminalInstance,
) -> Result<(), String> {
    let available = ui.available_size();
    let (rows, cols) = terminal_size(available.x, available.y);
    terminal.session.resize(rows, cols);

    let id = terminal_id(terminal.id);
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    if response.clicked() {
        response.request_focus();
    }

    if response.has_focus() {
        ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }

    let has_focus = response.has_focus();
    handle_mouse(terminal, ctx, rect, rows, cols)?;
    handle_keyboard(terminal, ctx, has_focus)?;
    handle_scroll(terminal, ctx, response, rows)?;

    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, TERMINAL_BACKGROUND);

    let screen = terminal.session.parser.screen();
    let cell_width = terminal_cell_width(ctx, &terminal.session.parser, &terminal.cwd);
    let cell_height = CELL_HEIGHT;
    let cols = cols.min(screen.size().1).max(1);
    let rows = rows.min(screen.size().0).max(1);

    for row in 0..rows {
        for col in 0..cols {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }

            let x = rect.left() + col as f32 * cell_width;
            let y = rect.top() + row as f32 * cell_height;
            let cell_rect = Rect::from_min_size(
                Pos2::new(x, y),
                egui::vec2(cell_width, cell_height),
            );

            let mut fg = color_to_egui(cell.fgcolor(), TERMINAL_FOREGROUND);
            let mut bg = color_to_egui(cell.bgcolor(), TERMINAL_BACKGROUND);
            if cell.inverse() {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.dim() {
                fg = fg.gamma_multiply(0.65);
            }
            if bg != TERMINAL_BACKGROUND {
                painter.rect_filled(cell_rect, 0.0, bg);
            }

            if cell.has_contents() {
                painter.text(
                    Pos2::new(x, y),
                    Align2::LEFT_TOP,
                    cell.contents(),
                    FontId::monospace(FONT_SIZE),
                    fg,
                );
            }
        }
    }

    let (cursor_row, cursor_col) = screen.cursor_position();
    if has_focus && !screen.hide_cursor() && cursor_row < rows && cursor_col < cols {
        let cursor_rect = Rect::from_min_size(
            Pos2::new(
                rect.left() + cursor_col as f32 * cell_width,
                rect.top() + cursor_row as f32 * cell_height,
            ),
            egui::vec2(cell_width.min(2.0), cell_height),
        );
        painter.rect_filled(cursor_rect, 0.0, CURSOR_COLOR);
    }

    if !terminal.session.is_alive() {
        painter.text(
            Pos2::new(rect.left() + 8.0, rect.bottom() - 24.0),
            Align2::LEFT_TOP,
            "Process exited - press + to start another terminal",
            FontId::proportional(12.0),
            Color32::from_rgb(130, 136, 148),
        );
    }

    ctx.request_repaint_after(Duration::from_millis(16));
    Ok(())
}

fn handle_keyboard(
    terminal: &mut TerminalInstance,
    ctx: &Context,
    focused: bool,
) -> Result<(), String> {
    if !focused {
        return Ok(());
    }

    let events = ctx.input(|input| input.events.clone());
    let application_cursor = terminal.session.parser.screen().application_cursor();
    let bracketed_paste = terminal.session.parser.screen().bracketed_paste();

    for event in events {
        match event {
            egui::Event::Text(text) => {
                if !text.is_empty() {
                    let bytes = if ctx.input(|input| input.modifiers.alt) {
                        let mut value = Vec::with_capacity(text.len() + 1);
                        value.push(0x1b);
                        value.extend_from_slice(text.as_bytes());
                        value
                    } else {
                        text.into_bytes()
                    };
                    terminal.session.write_bytes(&bytes)?;
                }
            }
            egui::Event::Paste(text) => {
                let bytes = if bracketed_paste {
                    let mut value = Vec::with_capacity(text.len() + 12);
                    value.extend_from_slice(b"\x1b[200~");
                    value.extend_from_slice(text.as_bytes());
                    value.extend_from_slice(b"\x1b[201~");
                    value
                } else {
                    text.into_bytes()
                };
                terminal.session.write_bytes(&bytes)?;
            }
            egui::Event::Key {
                key,
                pressed,
                repeat: _,
                modifiers,
                ..
            } if pressed => {
                if let Some(bytes) = key_bytes(key, modifiers, application_cursor) {
                    terminal.session.write_bytes(&bytes)?;
                }
            }
            egui::Event::Cut | egui::Event::Copy => {}
            _ => {}
        }
    }

    Ok(())
}

fn handle_scroll(
    terminal: &mut TerminalInstance,
    ctx: &Context,
    response: egui::Response,
    rows: u16,
) -> Result<(), String> {
    if !response.hovered() {
        return Ok(());
    }

    let scroll_y = ctx.input(|input| input.smooth_scroll_delta.y);
    if scroll_y.abs() < f32::EPSILON {
        return Ok(());
    }

    let screen = terminal.session.parser.screen_mut();
    let current = screen.scrollback();
    let amount = (scroll_y.abs() / rows.max(1) as f32).ceil() as usize;

    if scroll_y > 0.0 {
        screen.set_scrollback(current.saturating_add(amount));
    } else {
        screen.set_scrollback(current.saturating_sub(amount));
    }

    Ok(())
}

fn handle_mouse(
    terminal: &mut TerminalInstance,
    ctx: &Context,
    rect: Rect,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let events = ctx.input(|input| input.events.clone());
    let screen = terminal.session.parser.screen();
    let mode = screen.mouse_protocol_mode();
    let encoding = screen.mouse_protocol_encoding();
    if mode == vt100::MouseProtocolMode::None {
        return Ok(());
    }

    let cell_width = terminal_cell_width(ctx, &terminal.session.parser, &terminal.cwd);
    let cell_height = CELL_HEIGHT;

    for event in events {
        let egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: _,
        } = event else {
            continue;
        };
        if !rect.contains(pos) {
            continue;
        }

        let col = ((pos.x - rect.left()) / cell_width).floor() as u16;
        let row = ((pos.y - rect.top()) / cell_height).floor() as u16;
        if col >= cols || row >= rows {
            continue;
        }

        let Some(button_code) = mouse_button_code(button) else {
            continue;
        };

        if !pressed && mode == vt100::MouseProtocolMode::Press {
            continue;
        }

        let bytes = match encoding {
            vt100::MouseProtocolEncoding::Sgr => {
                let suffix = if pressed { 'M' } else { 'm' };
                format!("\x1b[<{};{};{}{}", button_code, col + 1, row + 1, suffix).into_bytes()
            }
            _ => {
                let code = button_code.saturating_add(32) as u8;
                vec![0x1b, b'[', b'M', code, (col + 33).min(255) as u8, (row + 33).min(255) as u8]
            }
        };

        terminal.session.write_bytes(&bytes)?;
    }

    Ok(())
}

fn key_bytes(
    key: egui::Key,
    modifiers: egui::Modifiers,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    if modifiers.ctrl {
        let ctrl = match key {
            egui::Key::A => 0x01,
            egui::Key::B => 0x02,
            egui::Key::C => 0x03,
            egui::Key::D => 0x04,
            egui::Key::E => 0x05,
            egui::Key::F => 0x06,
            egui::Key::G => 0x07,
            egui::Key::H => 0x08,
            egui::Key::I => 0x09,
            egui::Key::J => 0x0a,
            egui::Key::K => 0x0b,
            egui::Key::L => 0x0c,
            egui::Key::M => 0x0d,
            egui::Key::N => 0x0e,
            egui::Key::O => 0x0f,
            egui::Key::P => 0x10,
            egui::Key::Q => 0x11,
            egui::Key::R => 0x12,
            egui::Key::S => 0x13,
            egui::Key::T => 0x14,
            egui::Key::U => 0x15,
            egui::Key::V => 0x16,
            egui::Key::W => 0x17,
            egui::Key::X => 0x18,
            egui::Key::Y => 0x19,
            egui::Key::Z => 0x1a,
            egui::Key::Space => 0x00,
            egui::Key::Backslash => 0x1c,
            egui::Key::OpenBracket => 0x1b,
            egui::Key::CloseBracket => 0x1d,
            egui::Key::Enter => 0x0d,
            egui::Key::Backspace => 0x7f,
            _ => return None,
        };
        return Some(vec![ctrl]);
    }

    if modifiers.alt {
        if let Some(value) = key_text(key) {
            let mut bytes = Vec::with_capacity(value.len() + 1);
            bytes.push(0x1b);
            bytes.extend_from_slice(value.as_bytes());
            return Some(bytes);
        }
    }

    let bytes = match key {
        egui::Key::Enter => vec![b'\r'],
        egui::Key::Backspace => vec![0x7f],
        egui::Key::Tab if modifiers.shift => b"\x1b[Z".to_vec(),
        egui::Key::Tab => vec![b'\t'],
        egui::Key::Escape => vec![0x1b],
        egui::Key::ArrowUp => cursor_sequence(application_cursor, b'A'),
        egui::Key::ArrowDown => cursor_sequence(application_cursor, b'B'),
        egui::Key::ArrowRight => cursor_sequence(application_cursor, b'C'),
        egui::Key::ArrowLeft => cursor_sequence(application_cursor, b'D'),
        egui::Key::Home => b"\x1b[H".to_vec(),
        egui::Key::End => b"\x1b[F".to_vec(),
        egui::Key::PageUp => b"\x1b[5~".to_vec(),
        egui::Key::PageDown => b"\x1b[6~".to_vec(),
        egui::Key::Insert => b"\x1b[2~".to_vec(),
        egui::Key::Delete => b"\x1b[3~".to_vec(),
        egui::Key::F1 => b"\x1bOP".to_vec(),
        egui::Key::F2 => b"\x1bOQ".to_vec(),
        egui::Key::F3 => b"\x1bOR".to_vec(),
        egui::Key::F4 => b"\x1bOS".to_vec(),
        egui::Key::F5 => b"\x1b[15~".to_vec(),
        egui::Key::F6 => b"\x1b[17~".to_vec(),
        egui::Key::F7 => b"\x1b[18~".to_vec(),
        egui::Key::F8 => b"\x1b[19~".to_vec(),
        egui::Key::F9 => b"\x1b[20~".to_vec(),
        egui::Key::F10 => b"\x1b[21~".to_vec(),
        egui::Key::F11 => b"\x1b[23~".to_vec(),
        egui::Key::F12 => b"\x1b[24~".to_vec(),
        _ => return None,
    };

    Some(bytes)
}

fn key_text(key: egui::Key) -> Option<&'static str> {
    match key {
        egui::Key::Space => Some(" "),
        egui::Key::A => Some("a"),
        egui::Key::B => Some("b"),
        egui::Key::C => Some("c"),
        egui::Key::D => Some("d"),
        egui::Key::E => Some("e"),
        egui::Key::F => Some("f"),
        egui::Key::G => Some("g"),
        egui::Key::H => Some("h"),
        egui::Key::I => Some("i"),
        egui::Key::J => Some("j"),
        egui::Key::K => Some("k"),
        egui::Key::L => Some("l"),
        egui::Key::M => Some("m"),
        egui::Key::N => Some("n"),
        egui::Key::O => Some("o"),
        egui::Key::P => Some("p"),
        egui::Key::Q => Some("q"),
        egui::Key::R => Some("r"),
        egui::Key::S => Some("s"),
        egui::Key::T => Some("t"),
        egui::Key::U => Some("u"),
        egui::Key::V => Some("v"),
        egui::Key::W => Some("w"),
        egui::Key::X => Some("x"),
        egui::Key::Y => Some("y"),
        egui::Key::Z => Some("z"),
        _ => None,
    }
}

fn cursor_sequence(application_cursor: bool, final_byte: u8) -> Vec<u8> {
    if application_cursor {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

fn mouse_button_code(button: PointerButton) -> Option<u8> {
    match button {
        PointerButton::Primary => Some(0),
        PointerButton::Middle => Some(1),
        PointerButton::Secondary => Some(2),
        _ => None,
    }
}

fn terminal_size(width: f32, height: f32) -> (u16, u16) {
    let cols = (width / estimated_cell_width()).floor().max(1.0) as u16;
    let rows = (height / CELL_HEIGHT).floor().max(1.0) as u16;
    (rows, cols)
}

fn estimated_cell_width() -> f32 {
    FONT_SIZE * 0.62
}

fn terminal_cell_width(_ctx: &Context, _parser: &vt100::Parser, _cwd: &Path) -> f32 {
    estimated_cell_width()
}

fn color_to_egui(color: vt100::Color, default: Color32) -> Color32 {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Rgb(red, green, blue) => Color32::from_rgb(red, green, blue),
        vt100::Color::Idx(index) => ansi_color(index),
    }
}

fn ansi_color(index: u8) -> Color32 {
    const COLORS: [Color32; 16] = [
        Color32::from_rgb(29, 31, 36),
        Color32::from_rgb(224, 108, 117),
        Color32::from_rgb(152, 195, 121),
        Color32::from_rgb(229, 192, 123),
        Color32::from_rgb(97, 175, 239),
        Color32::from_rgb(198, 120, 221),
        Color32::from_rgb(86, 182, 194),
        Color32::from_rgb(171, 178, 191),
        Color32::from_rgb(92, 99, 112),
        Color32::from_rgb(240, 113, 120),
        Color32::from_rgb(165, 214, 130),
        Color32::from_rgb(238, 210, 140),
        Color32::from_rgb(124, 190, 255),
        Color32::from_rgb(214, 141, 246),
        Color32::from_rgb(110, 203, 214),
        Color32::from_rgb(210, 214, 224),
    ];

    if index < 16 {
        return COLORS[index as usize];
    }
    if index < 232 {
        let value = index - 16;
        let red = value / 36;
        let green = (value / 6) % 6;
        let blue = value % 6;
        let convert = |component: u8| -> u8 {
            if component == 0 {
                0
            } else {
                55 + component * 40
            }
        };
        return Color32::from_rgb(convert(red), convert(green), convert(blue));
    }

    let gray = 8 + (index - 232) * 10;
    Color32::from_gray(gray)
}

fn shell_command() -> Result<CommandBuilder, String> {
    #[cfg(windows)]
    {
        if let Some(shell) = env::var_os("ECode_TERMINAL_SHELL") {
            return Ok(CommandBuilder::new(shell));
        }
        let mut command = CommandBuilder::new("powershell.exe");
        command.arg("-NoLogo");
        return Ok(command);
    }

    #[cfg(not(windows))]
    {
        if let Some(shell) = env::var_os("ECode_TERMINAL_SHELL") {
            return Ok(CommandBuilder::new(shell));
        }
        Ok(CommandBuilder::new_default_prog())
    }
}

fn shell_display_name() -> String {
    #[cfg(windows)]
    {
        return String::from("PowerShell");
    }

    #[cfg(not(windows))]
    {
        env::var("SHELL")
            .ok()
            .and_then(|shell| PathBuf::from(shell).file_name().map(|name| name.to_string_lossy().to_string()))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| String::from("shell"))
    }
}

fn terminal_id(id: u64) -> Id {
    Id::new(("ecode_terminal", id))
}
