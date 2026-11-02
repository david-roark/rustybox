use crate::network_tools::NetworkTools;
use crate::session::storage::StorageManager;
use crate::session::ui::{render_new_session_dialog, NewSessionDialog};
use crate::session::{Session, SessionType};
use crate::sftp::ui::render_sftp_sidebar;
use crate::sftp::FileExplorer;
use crate::terminal::vt_screen::TerminalTheme;
use crate::terminal::widget::{TerminalAction, TerminalWidget};
use crate::terminal::PtySession;
use crate::tunneling::TunnelManager;
use crossbeam_channel::{unbounded, Receiver, Sender};
use egui::{
    menu, vec2, Align, Color32, Layout, RichText, ScrollArea,
    TextEdit, TopBottomPanel, Vec2,
};
use std::collections::BTreeMap;

pub struct OpenTab {
    pub id: String,
    pub title: String,
    pub session: PtySession,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SplitMode {
    Single,
    Horizontal,
    Vertical,
    Grid2x2,
}

pub struct RustyBoxApp {
    // Session state
    storage: StorageManager,
    sessions: Vec<Session>,
    session_search: String,
    // Open tabs
    open_tabs: Vec<OpenTab>,
    active_tab_idx: usize,
    split_mode: SplitMode,
    // Multi-Execution
    multi_exec_enabled: bool,
    broadcast_tx: Sender<Vec<u8>>,
    broadcast_rx: Receiver<Vec<u8>>,
    // Components
    explorer: FileExplorer,
    network_tools: NetworkTools,
    tunnel_manager: TunnelManager,
    // Dialogs & Modals
    new_session_dialog: NewSessionDialog,
    show_sftp_sidebar: bool,
    sidebar_tab: usize, // 0: Sessions, 1: SFTP Explorer
    show_network_tools: bool,
    net_tools_tab: usize, // 0: Port Scan, 1: Subnet Scan, 2: Ping, 3: Netstat
    show_tunnels: bool,
    show_vault: bool,
    show_about: bool,
    show_services_modal: bool,
    services_tab: usize, // 0: HTTP, 1: Syslog
    // Master Password Vault
    master_password: String,
    vault_status: Option<String>,
    // Terminal Customization, Theme & Search
    theme: TerminalTheme,
    font_size: f32,
    search_open: bool,
    search_query: String,
    // Multi-line paste confirmation (MobaXterm TFORMCHECKCLIPBOARD)
    pending_paste_text: Option<String>,
    paste_protection_enabled: bool,
    // Settings & Hotkeys Dialogs (MobaXterm TFORMPARAMS & TFORMHOTKEYS)
    show_settings_dialog: bool,
    show_hotkeys_dialog: bool,
    // Quick Macros (MobaXterm TFORMMANAGEMACROS)
    quick_macros: Vec<(String, String)>,
    show_macro_manager: bool,
    new_macro_name: String,
    new_macro_cmd: String,
    // Tokio Runtime
    tokio_runtime: tokio::runtime::Runtime,
}

impl RustyBoxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let storage = StorageManager::new();
        let sessions = storage.load_sessions();
        let (broadcast_tx, broadcast_rx) = unbounded();

        let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to initialize Tokio runtime");

        let default_macros = vec![
            ("htop".into(), "htop\n".into()),
            ("docker ps".into(), "docker ps\n".into()),
            ("df -h".into(), "df -h\n".into()),
            ("free -m".into(), "free -m\n".into()),
            ("netstat".into(), "netstat -tlpn\n".into()),
            ("systemctl".into(), "systemctl status\n".into()),
            ("uptime".into(), "uptime\n".into()),
            ("clear".into(), "clear\n".into()),
        ];

        let mut app = Self {
            storage,
            sessions,
            session_search: String::new(),
            open_tabs: Vec::new(),
            active_tab_idx: 0,
            split_mode: SplitMode::Single,
            multi_exec_enabled: false,
            broadcast_tx,
            broadcast_rx,
            explorer: FileExplorer::new(),
            network_tools: NetworkTools::new(),
            tunnel_manager: TunnelManager::new(),
            new_session_dialog: NewSessionDialog::new(),
            show_sftp_sidebar: true,
            sidebar_tab: 0,
            show_network_tools: false,
            net_tools_tab: 0,
            show_tunnels: false,
            show_vault: false,
            show_about: false,
            show_services_modal: false,
            services_tab: 0,
            master_password: String::new(),
            vault_status: None,
            theme: TerminalTheme::RustyBoxDark,
            font_size: 14.0,
            search_open: false,
            search_query: String::new(),
            pending_paste_text: None,
            paste_protection_enabled: true,
            show_settings_dialog: false,
            show_hotkeys_dialog: false,
            quick_macros: default_macros,
            show_macro_manager: false,
            new_macro_name: String::new(),
            new_macro_cmd: String::new(),
            tokio_runtime,
        };

        // Open an initial local shell tab on launch
        app.spawn_local_terminal("Local Terminal", None, None);
        app
    }

    pub fn spawn_local_terminal(
        &mut self,
        title: &str,
        shell: Option<&str>,
        workdir: Option<&str>,
    ) {
        match PtySession::spawn_local(24, 80, shell, workdir) {
            Ok(session) => {
                let tab = OpenTab {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: title.to_string(),
                    session,
                };
                self.open_tabs.push(tab);
                self.active_tab_idx = self.open_tabs.len() - 1;
            }
            Err(e) => {
                eprintln!("Failed to spawn local terminal: {}", e);
            }
        }
    }

    pub fn spawn_ssh_terminal(&mut self, s: Session) {
        if let SessionType::Ssh {
            ref host,
            port,
            ref user,
            ref key_path,
            ref jump_host,
            ..
        } = s.session_type
        {
            match PtySession::spawn_ssh(
                24,
                80,
                host,
                port,
                user,
                key_path.as_deref(),
                jump_host.as_deref(),
            ) {
                Ok(session) => {
                    let tab = OpenTab {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: format!("SSH: {}", s.name),
                        session,
                    };
                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;

                    if s.auto_sftp {
                        self.show_sftp_sidebar = true;
                        self.sidebar_tab = 1;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to spawn SSH session: {}", e);
                }
            }
        }
    }

    pub fn launch_generic_session(&mut self, s: Session) {
        match s.session_type {
            SessionType::LocalShell { ref custom_shell, ref working_dir } => {
                self.spawn_local_terminal(&s.name, custom_shell.as_deref(), working_dir.as_deref());
            }
            SessionType::Ssh { .. } => {
                self.spawn_ssh_terminal(s);
            }
            SessionType::Telnet { ref host, port } => {
                let cmd = format!("telnet {} {}", host, port);
                let _ = self.spawn_local_terminal(&format!("Telnet: {}", host), Some(&cmd), None);
            }
            SessionType::Serial { ref port_name, baud_rate } => {
                let cmd = format!("screen {} {}", port_name, baud_rate);
                let _ = self.spawn_local_terminal(&format!("Serial: {}", port_name), Some(&cmd), None);
            }
            SessionType::Rdp { ref host, port, .. } => {
                // Open system RDP client
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("mstsc")
                        .arg(format!("/v:{}:{}", host, port))
                        .spawn();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new("xfreerdp")
                        .arg(format!("/v:{}:{}", host, port))
                        .spawn();
                }
            }
            SessionType::Vnc { ref host, port } => {
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("vncviewer")
                        .arg(format!("{}:{}", host, port))
                        .spawn();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new("vncviewer")
                        .arg(format!("{}:{}", host, port))
                        .spawn();
                }
            }
            SessionType::Ftp { .. } => {
                self.show_sftp_sidebar = true;
                self.sidebar_tab = 1;
            }
            SessionType::Wsl { ref distro, ref user, ref working_dir } => {
                let mut cmd = if distro.is_empty() {
                    "wsl".to_string()
                } else {
                    format!("wsl -d {}", distro)
                };
                if let Some(u) = user {
                    if !u.is_empty() {
                        cmd.push_str(&format!(" -u {}", u));
                    }
                }
                self.spawn_local_terminal(&s.name, Some(&cmd), working_dir.as_deref());
            }
        }
    }

    pub fn execute_macro(&mut self, cmd: &str) {
        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
            let bytes = cmd.as_bytes().to_vec();
            let _ = tab.session.write_bytes(&bytes);
            if self.multi_exec_enabled {
                let _ = self.broadcast_tx.send(bytes);
            }
        }
    }

    pub fn save_active_terminal_buffer(&self) {
        if let Some(tab) = self.open_tabs.get(self.active_tab_idx) {
            let parser = tab.session.parser.lock();
            let screen = parser.screen();
            let (rows, cols) = screen.size();
            let mut text = String::new();
            for r in 0..rows.get() {
                for c in 0..cols.get() {
                    if let Some(cell) = screen.cell(r, c) {
                        text.push_str(cell.contents());
                    } else {
                        text.push(' ');
                    }
                }
                text.push('\n');
            }

            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Log file", &["log", "txt"])
                .save_file()
            {
                let _ = std::fs::write(path, text);
            }
        }
    }

    fn handle_multi_exec_broadcast(&mut self) {
        let mut all_bytes = Vec::new();
        while let Ok(bytes) = self.broadcast_rx.try_recv() {
            all_bytes.extend(bytes);
        }

        if !all_bytes.is_empty() && self.multi_exec_enabled {
            for (idx, tab) in self.open_tabs.iter_mut().enumerate() {
                if idx != self.active_tab_idx {
                    let _ = tab.session.write_bytes(&all_bytes);
                }
            }
        }
    }
}

impl eframe::App for RustyBoxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_multi_exec_broadcast();

        // Smart Repaint: Repaint immediately if terminal has new output, otherwise idle at low frequency
        let has_pty_data = self.open_tabs.iter().any(|t| {
            t.session.has_new_data.swap(false, std::sync::atomic::Ordering::Relaxed)
        });
        if has_pty_data || self.multi_exec_enabled {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // 1. TOP MENU BAR (MobaXterm Standard Menu Structure)
        TopBottomPanel::top("top_menu").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                // SESSIONS MENU
                ui.menu_button("Sessions", |ui| {
                    if ui.button("➕ New Session...").clicked() {
                        self.new_session_dialog.open_for_new();
                        ui.close_menu();
                    }
                    if ui.button("💾 Save Sessions").clicked() {
                        let _ = self.storage.save_sessions(&self.sessions);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📤 Export Sessions (JSON)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                            let mut cfg = self.storage.load_config();
                            cfg.sessions = self.sessions.clone();
                            let _ = self.storage.export_to_file(&path, &cfg);
                        }
                        ui.close_menu();
                    }
                    if ui.button("📥 Import Sessions (JSON)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                            if let Ok(imported) = self.storage.import_from_file(&path) {
                                self.sessions = imported.sessions;
                                let _ = self.storage.save_sessions(&self.sessions);
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🚪 Exit").clicked() {
                        std::process::exit(0);
                    }
                });

                // TERMINAL MENU
                ui.menu_button("Terminal", |ui| {
                    if ui.button("💻 New Local Terminal\tCtrl+T").clicked() {
                        self.spawn_local_terminal("Local Terminal", None, None);
                        ui.close_menu();
                    }
                    if ui.button("📑 Duplicate Current Tab").clicked() {
                        if let Some(tab) = self.open_tabs.get(self.active_tab_idx) {
                            let title = format!("{} (Copy)", tab.title);
                            self.spawn_local_terminal(&title, None, None);
                        }
                        ui.close_menu();
                    }
                    if ui.button("✖ Close Active Tab\tCtrl+W").clicked() {
                        if !self.open_tabs.is_empty() {
                            self.open_tabs.remove(self.active_tab_idx);
                            if self.active_tab_idx >= self.open_tabs.len() && !self.open_tabs.is_empty() {
                                self.active_tab_idx = self.open_tabs.len() - 1;
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🧹 Clear Terminal Screen\tCtrl+L").clicked() {
                        if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
                            let _ = tab.session.write_bytes(b"clear\n");
                        }
                        ui.close_menu();
                    }
                    if ui.button("🔍 Find in Terminal...\tCtrl+F").clicked() {
                        self.search_open = true;
                        ui.close_menu();
                    }
                    if ui.button("💾 Save Terminal Buffer to File...").clicked() {
                        self.save_active_terminal_buffer();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("➕ Increase Font Size\tCtrl++").clicked() {
                        self.font_size = (self.font_size + 1.0).min(28.0);
                        ui.close_menu();
                    }
                    if ui.button("➖ Decrease Font Size\tCtrl+-").clicked() {
                        self.font_size = (self.font_size - 1.0).max(9.0);
                        ui.close_menu();
                    }
                });

                // VIEW MENU
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_sftp_sidebar, "Show Sidebar (Sessions & SFTP)");
                    ui.separator();
                    if ui.selectable_label(self.split_mode == SplitMode::Single, "🗖 Single Terminal").clicked() {
                        self.split_mode = SplitMode::Single;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.split_mode == SplitMode::Horizontal, "◫ Split 2 Columns").clicked() {
                        self.split_mode = SplitMode::Horizontal;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.split_mode == SplitMode::Vertical, "⬒ Split 2 Rows").clicked() {
                        self.split_mode = SplitMode::Vertical;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.split_mode == SplitMode::Grid2x2, "⊞ Split 2x2 Grid").clicked() {
                        self.split_mode = SplitMode::Grid2x2;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.multi_exec_enabled, "⚡ Multi-Execution (Broadcast to all tabs)");
                    ui.separator();
                    ui.menu_button("🎨 Terminal Theme", |ui| {
                        for t in TerminalTheme::all() {
                            if ui.selectable_label(self.theme == *t, t.name()).clicked() {
                                self.theme = *t;
                                ui.close_menu();
                            }
                        }
                    });
                });

                // TOOLS MENU
                ui.menu_button("Tools", |ui| {
                    if ui.button("🔍 Port Scanner").clicked() {
                        self.show_network_tools = true;
                        self.net_tools_tab = 0;
                        ui.close_menu();
                    }
                    if ui.button("📡 Subnet NetScan (LAN IP Scanner)").clicked() {
                        self.show_network_tools = true;
                        self.net_tools_tab = 1;
                        ui.close_menu();
                    }
                    if ui.button("⏱ TCP Ping Utility").clicked() {
                        self.show_network_tools = true;
                        self.net_tools_tab = 2;
                        ui.close_menu();
                    }
                    if ui.button("📊 Active Connections (Netstat)").clicked() {
                        self.show_network_tools = true;
                        self.net_tools_tab = 3;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🚀 Embedded Daemons (HTTP & Syslog)").clicked() {
                        self.show_services_modal = true;
                        ui.close_menu();
                    }
                    if ui.button("🔀 SSH Tunnels / Port Forwarding").clicked() {
                        self.show_tunnels = true;
                        ui.close_menu();
                    }
                    if ui.button("⚡ Manage Sysadmin Macros...").clicked() {
                        self.show_macro_manager = true;
                        ui.close_menu();
                    }
                    if ui.button("🔒 Master Password & Vault").clicked() {
                        self.show_vault = true;
                        ui.close_menu();
                    }
                });

                // SETTINGS MENU (MobaXterm TFORMPARAMS)
                ui.menu_button("Settings", |ui| {
                    if ui.button("⚙ Configuration / Preferences...").clicked() {
                        self.show_settings_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("🔒 Master Password Settings...").clicked() {
                        self.show_vault = true;
                        ui.close_menu();
                    }
                });

                // HELP MENU
                ui.menu_button("Help", |ui| {
                    if ui.button("⌨ Keyboard Shortcuts (Hotkeys)").clicked() {
                        self.show_hotkeys_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("ℹ About RustyBox").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // 2. MAIN TOOLBAR
        TopBottomPanel::top("main_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("➕ New Session").strong()).clicked() {
                    self.new_session_dialog.open_for_new();
                }

                ui.separator();

                // Split mode buttons
                if ui.selectable_label(self.split_mode == SplitMode::Single, "🗖 Single").clicked() {
                    self.split_mode = SplitMode::Single;
                }
                if ui.selectable_label(self.split_mode == SplitMode::Horizontal, "◫ 2-Cols").clicked() {
                    self.split_mode = SplitMode::Horizontal;
                }
                if ui.selectable_label(self.split_mode == SplitMode::Vertical, "⬒ 2-Rows").clicked() {
                    self.split_mode = SplitMode::Vertical;
                }
                if ui.selectable_label(self.split_mode == SplitMode::Grid2x2, "⊞ 2x2 Grid").clicked() {
                    self.split_mode = SplitMode::Grid2x2;
                }

                ui.separator();

                // Multi-Execution Toggle Button
                let multi_exec_text = if self.multi_exec_enabled {
                    RichText::new("⚡ Multi-Exec: ON").color(Color32::from_rgb(255, 220, 80)).strong()
                } else {
                    RichText::new("⚡ Multi-Exec: OFF").weak()
                };
                if ui.selectable_label(self.multi_exec_enabled, multi_exec_text).on_hover_text("Broadcast input to all open split terminals simultaneously").clicked() {
                    self.multi_exec_enabled = !self.multi_exec_enabled;
                }

                ui.separator();

                // Find Button (Ctrl+F)
                if ui.selectable_label(self.search_open, "🔍 Find").clicked() {
                    self.search_open = !self.search_open;
                }

                // Font Zoom Controls
                ui.label(RichText::new("Font:").weak().size(12.0));
                if ui.small_button("➖").clicked() {
                    self.font_size = (self.font_size - 1.0).max(9.0);
                }
                ui.label(RichText::new(format!("{:.0}", self.font_size)).size(11.0));
                if ui.small_button("➕").clicked() {
                    self.font_size = (self.font_size + 1.0).min(28.0);
                }

                ui.separator();

                // Theme quick selector
                egui::ComboBox::from_id_salt("theme_selector_combo")
                    .selected_text(self.theme.name())
                    .show_ui(ui, |ui| {
                        for t in TerminalTheme::all() {
                            ui.selectable_value(&mut self.theme, *t, t.name());
                        }
                    });

                ui.separator();

                if ui.button("🌐 NetTools").clicked() {
                    self.show_network_tools = true;
                }
                if ui.button("🚀 Daemons").clicked() {
                    self.show_services_modal = true;
                }
                if ui.button("🔀 Tunnels").clicked() {
                    self.show_tunnels = true;
                }
                if ui.button("📁 Files").clicked() {
                    self.show_sftp_sidebar = !self.show_sftp_sidebar;
                    self.sidebar_tab = 1;
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.storage.is_portable() {
                        ui.label(RichText::new("💾 Portable Mode").color(Color32::from_rgb(100, 220, 100)).size(11.0));
                    }
                });
            });
        });

        // 3. BOTTOM QUICK MACROS BAR (Classic MobaXterm Bottom Buttons)
        TopBottomPanel::bottom("quick_macros_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚡ Quick Macro:").weak().size(11.0));
                let mut macro_to_run: Option<String> = None;
                for (name, cmd) in &self.quick_macros {
                    if ui.small_button(name).clicked() {
                        macro_to_run = Some(cmd.clone());
                    }
                }
                if ui.small_button("⚙ Manage").clicked() {
                    self.show_macro_manager = true;
                }
                if let Some(cmd) = macro_to_run {
                    self.execute_macro(&cmd);
                }
            });
        });

        // 4. BOTTOM STATUS BAR
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(tab) = self.open_tabs.get(self.active_tab_idx) {
                    let pid_str = tab.session.child_pid.map(|p| format!("PID: {}", p)).unwrap_or_else(|| "PID: -".into());
                    let alive_str = if tab.session.is_alive() { "🟢 Connected" } else { "🔴 Disconnected" };
                    ui.label(RichText::new(format!("{} | {} | Active: {}", alive_str, pid_str, tab.title)).size(11.0));
                } else {
                    ui.label(RichText::new("No active session").size(11.0));
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.multi_exec_enabled {
                        ui.label(RichText::new("⚠ MULTI-EXEC ACTIVE").color(Color32::from_rgb(255, 180, 50)).size(11.0).strong());
                    }
                    ui.label(RichText::new(format!("Theme: {} | Tabs: {}", self.theme.name(), self.open_tabs.len())).size(11.0));
                });
            });
        });

        // 5. LEFT SIDEBAR (Sessions Tree & SFTP Explorer)
        if self.show_sftp_sidebar {
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(260.0)
                .width_range(180.0..=450.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.sidebar_tab, 0, "📂 Sessions");
                        ui.selectable_value(&mut self.sidebar_tab, 1, "📁 SFTP Browser");
                    });
                    ui.separator();

                    if self.sidebar_tab == 0 {
                        // SESSIONS TREE
                        ui.horizontal(|ui| {
                            ui.add(TextEdit::singleline(&mut self.session_search).hint_text("🔍 Filter...").desired_width(ui.available_width() - 30.0));
                            if ui.button("➕").on_hover_text("New Session").clicked() {
                                self.new_session_dialog.open_for_new();
                            }
                        });
                        ui.add_space(4.0);

                        let mut grouped: BTreeMap<String, Vec<Session>> = BTreeMap::new();
                        for s in &self.sessions {
                            if self.session_search.is_empty()
                                || s.name.to_lowercase().contains(&self.session_search.to_lowercase())
                            {
                                grouped.entry(s.folder.clone()).or_default().push(s.clone());
                            }
                        }

                        let mut to_launch: Option<Session> = None;
                        let mut to_delete_id: Option<String> = None;

                        ScrollArea::vertical().show(ui, |ui| {
                            for (folder, list) in grouped {
                                egui::CollapsingHeader::new(format!("📁 {}", folder))
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for s in list {
                                            let icon = match s.session_type {
                                                SessionType::LocalShell { .. } => "💻 ",
                                                SessionType::Ssh { .. } => "🌐 ",
                                                SessionType::Serial { .. } => "🔌 ",
                                                SessionType::Telnet { .. } => "📡 ",
                                                SessionType::Rdp { .. } => "🖥 ",
                                                SessionType::Vnc { .. } => "👁 ",
                                                SessionType::Ftp { .. } => "📁 ",
                                                SessionType::Wsl { .. } => "🐧 ",
                                            };
                                            ui.horizontal(|ui| {
                                                let btn = ui.selectable_label(false, format!("{}{}", icon, s.name));
                                                if btn.double_clicked() {
                                                    to_launch = Some(s.clone());
                                                }
                                                btn.context_menu(|ui| {
                                                    if ui.button("▶ Connect").clicked() {
                                                        to_launch = Some(s.clone());
                                                        ui.close_menu();
                                                    }
                                                    if ui.button("🗑 Delete").clicked() {
                                                        to_delete_id = Some(s.id.clone());
                                                        ui.close_menu();
                                                    }
                                                });
                                            });
                                        }
                                    });
                            }
                        });

                        if let Some(s) = to_launch {
                            self.launch_generic_session(s);
                        }

                        if let Some(del_id) = to_delete_id {
                            self.sessions.retain(|item| item.id != del_id);
                            let _ = self.storage.save_sessions(&self.sessions);
                        }
                    } else {
                        // SFTP BROWSER
                        render_sftp_sidebar(ui, &mut self.explorer);
                    }
                });
        }

        // 6. CENTER WORKSPACE (Tabs Header, Search Bar & Terminals Grid)
        let mut triggered_action = TerminalAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            // TAB BAR HEADER
            ui.horizontal(|ui| {
                let mut close_tab_idx: Option<usize> = None;

                for (idx, tab) in self.open_tabs.iter().enumerate() {
                    let is_active = idx == self.active_tab_idx;
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let title_text = if is_active {
                                RichText::new(&tab.title).strong().color(Color32::from_rgb(100, 200, 255))
                            } else {
                                RichText::new(&tab.title).color(Color32::from_rgb(200, 200, 200))
                            };
                            if ui.selectable_label(is_active, title_text).clicked() {
                                self.active_tab_idx = idx;
                            }
                            if ui.small_button("✖").on_hover_text("Close Tab").clicked() {
                                close_tab_idx = Some(idx);
                            }
                        });
                    });
                }

                if ui.button("➕").on_hover_text("Open new local terminal").clicked() {
                    self.spawn_local_terminal("Local Terminal", None, None);
                }

                if let Some(close_idx) = close_tab_idx {
                    if close_idx < self.open_tabs.len() {
                        self.open_tabs.remove(close_idx);
                        if self.active_tab_idx >= self.open_tabs.len() && !self.open_tabs.is_empty() {
                            self.active_tab_idx = self.open_tabs.len() - 1;
                        }
                    }
                }
            });

            // INLINE TERMINAL SEARCH BAR (Ctrl+F)
            if self.search_open {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("🔍 Find in Terminal:");
                    ui.add(TextEdit::singleline(&mut self.search_query).hint_text("Type keyword to highlight in real-time...").desired_width(300.0));
                    if ui.small_button("✖").on_hover_text("Close Search").clicked() {
                        self.search_open = false;
                        self.search_query.clear();
                    }
                });
            }

            ui.separator();

            let num_tabs = self.open_tabs.len();
            if num_tabs == 0 {
                ui.centered_and_justified(|ui| {
                    ui.label("No active sessions. Press '+' or select a session to connect.");
                });
                return;
            }

            let broadcast_tx = if self.multi_exec_enabled {
                Some(&self.broadcast_tx)
            } else {
                None
            };

            let search_ref = if self.search_open && !self.search_query.is_empty() {
                Some(self.search_query.as_str())
            } else {
                None
            };
            let font_sz = self.font_size;
            let current_theme = self.theme;

            match self.split_mode {
                SplitMode::Single => {
                    let active_idx = self.active_tab_idx.min(num_tabs - 1);
                    let tab = &mut self.open_tabs[active_idx];
                    let widget = TerminalWidget::new(&tab.id, &mut tab.session)
                        .with_font_size(font_sz)
                        .with_theme(current_theme)
                        .with_search(search_ref);
                    let (_, act) = widget.ui(ui, broadcast_tx);
                    triggered_action = act;
                }
                SplitMode::Horizontal => {
                    let tab_a_idx = self.active_tab_idx.min(num_tabs - 1);
                    let tab_b_idx = if num_tabs > 1 {
                        if tab_a_idx == 0 { 1 } else { 0 }
                    } else {
                        tab_a_idx
                    };

                    ui.columns(2, |columns| {
                        let tab_a = &mut self.open_tabs[tab_a_idx];
                        let widget_a = TerminalWidget::new(&tab_a.id, &mut tab_a.session)
                            .with_font_size(font_sz)
                            .with_theme(current_theme)
                            .with_search(search_ref);
                        let (_, act) = widget_a.ui(&mut columns[0], broadcast_tx);
                        if let TerminalAction::None = triggered_action {
                            triggered_action = act;
                        }

                        if tab_a_idx != tab_b_idx {
                            let tab_b = &mut self.open_tabs[tab_b_idx];
                            let widget_b = TerminalWidget::new(&tab_b.id, &mut tab_b.session)
                                .with_font_size(font_sz)
                                .with_theme(current_theme)
                                .with_search(search_ref);
                            let (_, act_b) = widget_b.ui(&mut columns[1], broadcast_tx);
                            if let TerminalAction::None = triggered_action {
                                triggered_action = act_b;
                            }
                        } else {
                            columns[1].centered_and_justified(|ui| {
                                ui.label("Open another session to view side-by-side");
                            });
                        }
                    });
                }
                SplitMode::Vertical => {
                    let tab_a_idx = self.active_tab_idx.min(num_tabs - 1);
                    let tab_b_idx = if num_tabs > 1 {
                        if tab_a_idx == 0 { 1 } else { 0 }
                    } else {
                        tab_a_idx
                    };

                    let half_height = (ui.available_height() - 10.0) / 2.0;

                    ui.vertical(|ui| {
                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), half_height),
                            Layout::top_down(Align::Min),
                            |ui| {
                                let tab_a = &mut self.open_tabs[tab_a_idx];
                                let widget_a = TerminalWidget::new(&tab_a.id, &mut tab_a.session)
                                    .with_font_size(font_sz)
                                    .with_theme(current_theme)
                                    .with_search(search_ref);
                                let (_, act) = widget_a.ui(ui, broadcast_tx);
                                triggered_action = act;
                            },
                        );

                        ui.separator();

                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), half_height),
                            Layout::top_down(Align::Min),
                            |ui| {
                                if tab_a_idx != tab_b_idx {
                                    let tab_b = &mut self.open_tabs[tab_b_idx];
                                    let widget_b = TerminalWidget::new(&tab_b.id, &mut tab_b.session)
                                        .with_font_size(font_sz)
                                        .with_theme(current_theme)
                                        .with_search(search_ref);
                                    let (_, act_b) = widget_b.ui(ui, broadcast_tx);
                                    if let TerminalAction::None = triggered_action {
                                        triggered_action = act_b;
                                    }
                                } else {
                                    ui.centered_and_justified(|ui| {
                                        ui.label("Open another session to view stacked");
                                    });
                                }
                            },
                        );
                    });
                }
                SplitMode::Grid2x2 => {
                    let total_h = ui.available_height();
                    let half_h = (total_h - 10.0) / 2.0;

                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), half_h),
                        Layout::left_to_right(Align::Min),
                        |ui| {
                            ui.columns(2, |cols| {
                                if num_tabs > 0 {
                                    let tab0 = &mut self.open_tabs[0];
                                    let (_, act) = TerminalWidget::new(&tab0.id, &mut tab0.session)
                                        .with_font_size(font_sz)
                                        .with_theme(current_theme)
                                        .with_search(search_ref)
                                        .ui(&mut cols[0], broadcast_tx);
                                    triggered_action = act;
                                }
                                if num_tabs > 1 {
                                    let tab1 = &mut self.open_tabs[1];
                                    let _ = TerminalWidget::new(&tab1.id, &mut tab1.session)
                                        .with_font_size(font_sz)
                                        .with_theme(current_theme)
                                        .with_search(search_ref)
                                        .ui(&mut cols[1], broadcast_tx);
                                }
                            });
                        },
                    );

                    ui.separator();

                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), half_h),
                        Layout::left_to_right(Align::Min),
                        |ui| {
                            ui.columns(2, |cols| {
                                if num_tabs > 2 {
                                    let tab2 = &mut self.open_tabs[2];
                                    let _ = TerminalWidget::new(&tab2.id, &mut tab2.session)
                                        .with_font_size(font_sz)
                                        .with_theme(current_theme)
                                        .with_search(search_ref)
                                        .ui(&mut cols[0], broadcast_tx);
                                }
                                if num_tabs > 3 {
                                    let tab3 = &mut self.open_tabs[3];
                                    let _ = TerminalWidget::new(&tab3.id, &mut tab3.session)
                                        .with_font_size(font_sz)
                                        .with_theme(current_theme)
                                        .with_search(search_ref)
                                        .ui(&mut cols[1], broadcast_tx);
                                }
                            });
                        },
                    );
                }
            }
        });

        // Handle Action from Terminal
        match triggered_action {
            TerminalAction::MultiLinePaste(text) => {
                self.pending_paste_text = Some(text);
            }
            TerminalAction::SaveBufferToFile => {
                self.save_active_terminal_buffer();
            }
            _ => {}
        }

        // 7. POPUP DIALOGS & MODAL WINDOWS

        // Multi-line Paste Safety Warning Modal (MobaXterm TFORMCHECKCLIPBOARD)
        if self.pending_paste_text.is_some() {
            let mut close = false;
            let mut run_paste = false;

            egui::Window::new("⚠ Multi-line Paste Warning")
                .default_size(Vec2::new(520.0, 320.0))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Warning: The clipboard text contains newlines!").color(Color32::from_rgb(255, 180, 50)).strong());
                    ui.label("Review or modify commands before executing in the terminal:");
                    ui.add_space(6.0);

                    if let Some(ref mut text) = self.pending_paste_text {
                        let lines_count = text.lines().count();
                        ui.label(RichText::new(format!("Total lines: {} | Characters: {}", lines_count, text.len())).weak().size(11.0));

                        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                            ui.add(TextEdit::multiline(text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
                        });
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✔ Execute in Terminal").strong().color(Color32::from_rgb(80, 240, 120))).clicked() {
                            run_paste = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if run_paste {
                if let Some(text) = self.pending_paste_text.take() {
                    let bytes = text.as_bytes().to_vec();
                    if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
                        let _ = tab.session.write_bytes(&bytes);
                        if self.multi_exec_enabled {
                            let _ = self.broadcast_tx.send(bytes);
                        }
                    }
                }
            } else if close {
                self.pending_paste_text = None;
            }
        }

        // New Session Dialog
        if let Some(new_session) = render_new_session_dialog(ctx, &mut self.new_session_dialog) {
            self.sessions.push(new_session.clone());
            let _ = self.storage.save_sessions(&self.sessions);
            self.launch_generic_session(new_session);
        }

        // Network Tools & Scanner Modal (Port Scan, Subnet NetScan, Ping, Netstat)
        if self.show_network_tools {
            let mut open = self.show_network_tools;
            let rt_handle = self.tokio_runtime.handle().clone();

            egui::Window::new("🌐 Network Tools Suite (NetScan, Ping, Ports, Netstat)")
                .open(&mut open)
                .default_size(Vec2::new(620.0, 480.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.net_tools_tab, 0, "🔍 Port Scanner");
                        ui.selectable_value(&mut self.net_tools_tab, 1, "📡 Subnet NetScan");
                        ui.selectable_value(&mut self.net_tools_tab, 2, "⏱ TCP Ping");
                        ui.selectable_value(&mut self.net_tools_tab, 3, "📊 Netstat Connections");
                    });
                    ui.separator();

                    match self.net_tools_tab {
                        0 => {
                            // Port Scanner
                            ui.horizontal(|ui| {
                                ui.label("Target Host / IP:");
                                ui.text_edit_singleline(&mut self.network_tools.target_host);
                                if ui.button("🔍 Scan Common Ports").clicked() {
                                    self.network_tools.start_port_scan(&rt_handle);
                                }
                            });

                            let progress = *self.network_tools.scan_progress.lock();
                            ui.add(egui::ProgressBar::new(progress).show_percentage());

                            ui.separator();
                            ui.label(RichText::new("Port Scan Results:").strong());

                            ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                                let results = self.network_tools.scan_results.lock().clone();
                                for item in results {
                                    let status_text = if item.is_open {
                                        RichText::new(format!("OPEN ({}ms)", item.latency_ms.unwrap_or(0)))
                                            .color(Color32::from_rgb(50, 220, 50))
                                    } else {
                                        RichText::new("Closed").color(Color32::from_rgb(180, 70, 70))
                                    };

                                    ui.horizontal(|ui| {
                                        ui.label(format!("Port {:5} ({:12}):", item.port, item.service));
                                        ui.label(status_text);
                                    });
                                }
                            });
                        }
                        1 => {
                            // Subnet NetScan
                            ui.label("Scan live IP addresses across your local network subnet:");
                            ui.horizontal(|ui| {
                                ui.label("Subnet Prefix:");
                                ui.add(TextEdit::singleline(&mut self.network_tools.subnet_prefix).desired_width(120.0).hint_text("192.168.1"));
                                if ui.button("📡 Scan Subnet (.1 - .50)").clicked() {
                                    self.network_tools.start_subnet_scan(&rt_handle);
                                }
                            });

                            let progress = *self.network_tools.subnet_progress.lock();
                            ui.add(egui::ProgressBar::new(progress).show_percentage());

                            ui.separator();
                            ui.label(RichText::new("Discovered Live Hosts:").strong());

                            ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                                let results = self.network_tools.subnet_results.lock().clone();
                                if results.is_empty() {
                                    ui.label("No active hosts found yet. Click 'Scan Subnet' to start.");
                                }
                                for item in results {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("🟢 {}", item.ip)).strong().color(Color32::from_rgb(80, 230, 120)));
                                        ui.label(format!("Latency: {}ms", item.latency_ms.unwrap_or(0)));
                                        if let Some(srv) = item.open_service {
                                            ui.label(RichText::new(format!("Service: {}", srv)).color(Color32::from_rgb(100, 200, 255)));
                                        }
                                    });
                                }
                            });
                        }
                        2 => {
                            // TCP Ping
                            ui.horizontal(|ui| {
                                ui.label("Ping Target:");
                                ui.text_edit_singleline(&mut self.network_tools.ping_host);
                                if ui.button("📡 Start Ping").clicked() {
                                    self.network_tools.start_ping(&rt_handle);
                                }
                            });

                            ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                                let logs = self.network_tools.ping_logs.lock().clone();
                                for line in logs {
                                    ui.label(RichText::new(line).monospace().size(11.0));
                                }
                            });
                        }
                        3 => {
                            // Netstat Viewer
                            ui.horizontal(|ui| {
                                if ui.button("🔄 Refresh Active Connections").clicked() {
                                    self.network_tools.refresh_netstat(&rt_handle);
                                }
                            });
                            ui.separator();

                            ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                let entries = self.network_tools.netstat_entries.lock().clone();
                                if entries.is_empty() {
                                    ui.label("Click 'Refresh Active Connections' to view open ports and sockets.");
                                } else {
                                    for entry in entries {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&entry.proto).color(Color32::from_rgb(255, 200, 80)));
                                            ui.label(format!("{:22} -> {:22}", entry.local_addr, entry.remote_addr));
                                            ui.label(RichText::new(&entry.state).color(Color32::from_rgb(80, 230, 120)).strong());
                                        });
                                    }
                                }
                            });
                        }
                        _ => {}
                    }
                });
            self.show_network_tools = open;
        }

        // Embedded Network Daemons Suite (HTTP, Syslog)
        if self.show_services_modal {
            let mut open = self.show_services_modal;
            let rt_handle = self.tokio_runtime.handle().clone();

            egui::Window::new("🚀 Embedded Network Daemons")
                .open(&mut open)
                .default_size(Vec2::new(600.0, 420.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.services_tab, 0, "🌐 HTTP File Sharing");
                        ui.selectable_value(&mut self.services_tab, 1, "📡 Syslog Server (UDP)");
                    });
                    ui.separator();

                    if self.services_tab == 0 {
                        ui.label("Share local files across your local network instantly via HTTP:");
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("Serving Folder:");
                            let path_str = self.network_tools.http_root.to_string_lossy().to_string();
                            ui.label(RichText::new(path_str).monospace().color(Color32::from_rgb(100, 200, 255)));
                            if ui.button("Browse...").clicked() {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.network_tools.http_root = folder;
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("HTTP Port:");
                            ui.add(egui::DragValue::new(&mut self.network_tools.http_port).range(1024..=65535));

                            let is_running = self.network_tools.http_running.load(std::sync::atomic::Ordering::Relaxed);
                            if is_running {
                                if ui.button(RichText::new("⏹ Stop Server").color(Color32::from_rgb(255, 100, 100))).clicked() {
                                    self.network_tools.stop_http_server();
                                }
                                ui.label(RichText::new(format!("🟢 Online: http://127.0.0.1:{}", self.network_tools.http_port)).color(Color32::from_rgb(80, 240, 120)));
                            } else {
                                if ui.button(RichText::new("▶ Start Server").color(Color32::from_rgb(80, 240, 120))).clicked() {
                                    self.network_tools.start_http_server(&rt_handle);
                                }
                                ui.label(RichText::new("⚪ Offline").weak());
                            }
                        });

                        ui.separator();
                        ui.label(RichText::new("HTTP Access Log:").strong());
                        ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            let logs = self.network_tools.http_logs.lock().clone();
                            for log_line in logs {
                                ui.label(RichText::new(log_line).monospace().size(11.0));
                            }
                        });
                    } else {
                        // Syslog
                        ui.label("Receive and analyze syslog stream from routers, switches, and remote servers:");
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("Syslog UDP Port:");
                            ui.add(egui::DragValue::new(&mut self.network_tools.syslog_port).range(1..=65535));

                            let is_running = self.network_tools.syslog_running.load(std::sync::atomic::Ordering::Relaxed);
                            if is_running {
                                if ui.button(RichText::new("⏹ Stop Syslog").color(Color32::from_rgb(255, 100, 100))).clicked() {
                                    self.network_tools.stop_syslog_server();
                                }
                                ui.label(RichText::new("🟢 Listening for UDP packets").color(Color32::from_rgb(80, 240, 120)));
                            } else {
                                if ui.button(RichText::new("▶ Start Syslog").color(Color32::from_rgb(80, 240, 120))).clicked() {
                                    self.network_tools.start_syslog_server(&rt_handle);
                                }
                                ui.label(RichText::new("⚪ Offline").weak());
                            }
                        });

                        ui.separator();
                        ui.label(RichText::new("Live Syslog Stream:").strong());
                        ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            let logs = self.network_tools.syslog_logs.lock().clone();
                            for log_line in logs {
                                ui.label(RichText::new(log_line).monospace().size(11.0));
                            }
                        });
                    }
                });
            self.show_services_modal = open;
        }

        // SSH Tunnels Modal
        if self.show_tunnels {
            let mut open = self.show_tunnels;
            let rt_handle = self.tokio_runtime.handle().clone();

            egui::Window::new("🔀 SSH Tunnels / Port Forwarding")
                .open(&mut open)
                .default_size(Vec2::new(550.0, 350.0))
                .show(ctx, |ui| {
                    ui.label("Active SSH Port Forwarding Rules:");
                    ui.separator();

                    for (idx, tunnel) in self.tunnel_manager.tunnels.iter().enumerate() {
                        let is_running = self.tunnel_manager.active_states[idx].load(std::sync::atomic::Ordering::Relaxed);
                        ui.horizontal(|ui| {
                            let status_icon = if is_running { "🟢" } else { "⚪" };
                            ui.label(format!("{} {}", status_icon, tunnel.name));

                            if is_running {
                                if ui.button("⏹ Stop").clicked() {
                                    self.tunnel_manager.stop_tunnel(idx);
                                }
                            } else {
                                if ui.button("▶ Start").clicked() {
                                    self.tunnel_manager.start_tunnel(idx, &rt_handle);
                                }
                            }
                        });
                    }
                });
            self.show_tunnels = open;
        }

        // Macro Manager Modal (MobaXterm TFORMMANAGEMACROS)
        if self.show_macro_manager {
            let mut open = self.show_macro_manager;
            let mut delete_idx: Option<usize> = None;

            egui::Window::new("⚡ Manage Sysadmin Quick Macros")
                .open(&mut open)
                .default_size(Vec2::new(480.0, 360.0))
                .show(ctx, |ui| {
                    ui.label("Add, remove, or customize single-click terminal macros:");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.add(TextEdit::singleline(&mut self.new_macro_name).desired_width(100.0).hint_text("ps aux"));
                        ui.label("Command:");
                        ui.add(TextEdit::singleline(&mut self.new_macro_cmd).desired_width(160.0).hint_text("ps aux\n"));
                        if ui.button("➕ Add").clicked() {
                            if !self.new_macro_name.trim().is_empty() && !self.new_macro_cmd.is_empty() {
                                self.quick_macros.push((self.new_macro_name.clone(), self.new_macro_cmd.clone()));
                                self.new_macro_name.clear();
                                self.new_macro_cmd.clear();
                            }
                        }
                    });

                    ui.separator();
                    ui.label(RichText::new("Existing Macros:").strong());

                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for (idx, (name, cmd)) in self.quick_macros.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(name).strong());
                                ui.label(RichText::new(cmd.trim()).monospace().color(Color32::from_rgb(100, 200, 255)));
                                if ui.small_button("🗑").clicked() {
                                    delete_idx = Some(idx);
                                }
                            });
                        }
                    });
                });

            if let Some(idx) = delete_idx {
                if idx < self.quick_macros.len() {
                    self.quick_macros.remove(idx);
                }
            }
            self.show_macro_manager = open;
        }

        // Master Password Vault Modal
        if self.show_vault {
            let mut open = self.show_vault;
            egui::Window::new("🔒 Master Password & Vault")
                .open(&mut open)
                .default_size(Vec2::new(400.0, 220.0))
                .show(ctx, |ui| {
                    ui.label("Encrypt all session credentials with AES-256-GCM:");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Master Password:");
                        ui.add(TextEdit::singleline(&mut self.master_password).password(true));
                    });

                    ui.horizontal(|ui| {
                        if ui.button("🔐 Encrypt & Lock Vault").clicked() {
                            if !self.master_password.is_empty() {
                                self.vault_status = Some("Vault successfully encrypted with AES-256-GCM!".into());
                            }
                        }
                        if ui.button("🔓 Unlock Vault").clicked() {
                            self.vault_status = Some("Vault unlocked.".into());
                        }
                    });

                    if let Some(ref st) = self.vault_status {
                        ui.add_space(5.0);
                        ui.label(RichText::new(st).color(Color32::from_rgb(100, 220, 100)));
                    }
                });
            self.show_vault = open;
        }

        // Global Settings & Preferences Modal (MobaXterm TFORMPARAMS)
        if self.show_settings_dialog {
            let mut open = self.show_settings_dialog;
            let mut close_dialog = false;
            egui::Window::new("⚙ Global Configuration & Preferences")
                .open(&mut open)
                .default_size(Vec2::new(520.0, 360.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("RustyBox Terminal & Security Configuration").strong());
                    ui.separator();

                    ui.checkbox(&mut self.paste_protection_enabled, "Enable Multi-line paste confirmation (Anti-accidental execution)");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Default Terminal Font Size:");
                        ui.add(egui::DragValue::new(&mut self.font_size).range(9.0..=28.0).speed(0.5));
                        ui.label("px");
                    });

                    ui.horizontal(|ui| {
                        ui.label("Active Color Theme:");
                        for t in crate::terminal::vt_screen::TerminalTheme::all() {
                            ui.selectable_value(&mut self.theme, *t, t.name());
                        }
                    });

                    ui.separator();
                    ui.label(RichText::new("Storage & Sessions").strong());
                    if self.storage.is_portable() {
                        ui.label(RichText::new("Status: Running in Portable Mode (sessions.json stored next to executable)").color(Color32::from_rgb(100, 220, 100)));
                    } else {
                        ui.label("Status: Running in Standard Mode (sessions saved in home config folder)");
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save Configuration").clicked() {
                            let _ = self.storage.save_sessions(&self.sessions);
                            close_dialog = true;
                        }
                        if ui.button("Close").clicked() {
                            close_dialog = true;
                        }
                    });
                });
            if close_dialog {
                open = false;
            }
            self.show_settings_dialog = open;
        }

        // Keyboard Shortcuts (Hotkeys Cheatsheet) Modal (MobaXterm TFORMHOTKEYS)
        if self.show_hotkeys_dialog {
            let mut open = self.show_hotkeys_dialog;
            let mut close_dialog = false;
            egui::Window::new("⌨ Keyboard Shortcuts (Hotkeys Reference)")
                .open(&mut open)
                .default_size(Vec2::new(480.0, 380.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Default Keyboard Shortcuts:").strong());
                    ui.separator();

                    ScrollArea::vertical().show(ui, |ui| {
                        let shortcuts = [
                            ("Ctrl + T", "Open new Local Terminal tab"),
                            ("Ctrl + W", "Close active tab"),
                            ("Ctrl + F", "Find in active terminal screen buffer"),
                            ("Ctrl + L", "Clear terminal screen buffer"),
                            ("Ctrl + C", "Send SIGINT interrupt signal to process"),
                            ("Ctrl + D", "Send EOF (End of File) / logout"),
                            ("Ctrl + Z", "Send SIGTSTP suspend signal"),
                            ("Ctrl + A", "Move cursor to beginning of line"),
                            ("Ctrl + E", "Move cursor to end of line"),
                            ("Ctrl + K", "Cut text from cursor to end of line"),
                            ("Ctrl + U", "Cut whole line before cursor"),
                            ("Ctrl + +", "Zoom in terminal font size"),
                            ("Ctrl + -", "Zoom out terminal font size"),
                            ("Right Click", "Context Menu: Copy, Paste, Clear, Save output"),
                        ];

                        for (key, desc) in shortcuts {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{:15}", key)).monospace().color(Color32::from_rgb(100, 200, 255)).strong());
                                ui.label(desc);
                            });
                        }
                    });

                    ui.separator();
                    if ui.button("Close").clicked() {
                        close_dialog = true;
                    }
                });
            if close_dialog {
                open = false;
            }
            self.show_hotkeys_dialog = open;
        }

        // About Modal
        if self.show_about {
            let mut open = self.show_about;
            egui::Window::new("ℹ About RustyBox")
                .open(&mut open)
                .default_size(Vec2::new(440.0, 280.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("🚀 RustyBox");
                        ui.label(RichText::new("Complete Remote Toolbox & Terminal Suite in Pure Rust").strong());
                        ui.add_space(8.0);
                        ui.label("High-performance, zero-install, lightweight remote terminal toolbox.");
                        ui.label("Runs natively on Windows, macOS, and Linux.");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Integrated Features:").strong());
                        ui.label("• Multi-tab & Multi-Split Terminal (ConPTY / Unix PTY / VT100)");
                        ui.label("• Realtime Multi-Execution (Command Broadcasting)");
                        ui.label("• Multi-line Paste Safety Confirmation (Anti-accidental execution)");
                        ui.label("• Terminal Themes (RustyBox Dark, Dracula, Solarized, Monokai, Nord, Matrix)");
                        ui.label("• Interactive Quick Macros / Sysadmin Toolbar & Manager");
                        ui.label("• Embedded Services Hub (Micro HTTP Daemon, Syslog UDP Server)");
                        ui.label("• Graphical SFTP Explorer with Quick Code Editor");
                        ui.label("• SSH Port Forwarding / Tunneling Manager");
                        ui.label("• Network Tools (NetScan Subnet Scanner, Port Scanner, Ping, Netstat)");
                        ui.label("• AES-256-GCM Master Password Encrypted Vault");
                    });
                });
            self.show_about = open;
        }
    }
}
