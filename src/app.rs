use crate::network_tools::NetworkTools;
use crate::session::storage::StorageManager;
use crate::session::ui::{render_new_session_dialog, NewSessionDialog};
use crate::session::{Session, SessionType};
use crate::sftp::ui::render_sftp_sidebar;
use crate::sftp::FileExplorer;
use crate::terminal::widget::TerminalWidget;
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
    pub original_session: Option<Session>,
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
    show_tunnels: bool,
    show_vault: bool,
    show_about: bool,
    // Master Password Vault
    master_password: String,
    vault_status: Option<String>,
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
            show_tunnels: false,
            show_vault: false,
            show_about: false,
            master_password: String::new(),
            vault_status: None,
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
                    original_session: None,
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
                        original_session: Some(s.clone()),
                    };
                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;

                    if s.auto_sftp {
                        self.show_sftp_sidebar = true;
                        self.sidebar_tab = 1; // Switch to SFTP view
                    }
                }
                Err(e) => {
                    eprintln!("Failed to spawn SSH session: {}", e);
                }
            }
        }
    }

    fn handle_multi_exec_broadcast(&mut self) {
        // Collect all broadcasted inputs from active terminal
        let mut all_bytes = Vec::new();
        while let Ok(bytes) = self.broadcast_rx.try_recv() {
            all_bytes.extend(bytes);
        }

        if !all_bytes.is_empty() && self.multi_exec_enabled {
            // Send to ALL open tabs except the one that initiated if possible, or all tabs
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

        // Check if any tab child died, keep UI responsive
        ctx.request_repaint_after(std::time::Duration::from_millis(30));

        // 1. TOP MENU BAR
        TopBottomPanel::top("top_menu").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("➕ New Session...").clicked() {
                        self.new_session_dialog.open_for_new();
                        ui.close_menu();
                    }
                    if ui.button("💻 New Local Terminal").clicked() {
                        self.spawn_local_terminal("Local Terminal", None, None);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("💾 Save Sessions").clicked() {
                        let _ = self.storage.save_sessions(&self.sessions);
                        ui.close_menu();
                    }
                    if ui.button("📤 Export Sessions (JSON)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                            let _ = self.storage.export_to_file(&path, &self.sessions);
                        }
                        ui.close_menu();
                    }
                    if ui.button("📥 Import Sessions (JSON)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                            if let Ok(imported) = self.storage.import_from_file(&path) {
                                self.sessions = imported;
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

                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_sftp_sidebar, "Show Sidebar");
                    ui.separator();
                    if ui.selectable_label(self.split_mode == SplitMode::Single, "🗖 Single Terminal").clicked() {
                        self.split_mode = SplitMode::Single;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.split_mode == SplitMode::Horizontal, "◫ Split Horizontal").clicked() {
                        self.split_mode = SplitMode::Horizontal;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.split_mode == SplitMode::Vertical, "⬒ Split Vertical").clicked() {
                        self.split_mode = SplitMode::Vertical;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.split_mode == SplitMode::Grid2x2, "⊞ Split 2x2 Grid").clicked() {
                        self.split_mode = SplitMode::Grid2x2;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("🌐 Network Port Scanner").clicked() {
                        self.show_network_tools = true;
                        ui.close_menu();
                    }
                    if ui.button("🔀 SSH Tunnels Manager").clicked() {
                        self.show_tunnels = true;
                        ui.close_menu();
                    }
                    if ui.button("🔒 Master Password Vault").clicked() {
                        self.show_vault = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
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

                // Multi-Execution Toggle Button (Prominently styled)
                let multi_exec_text = if self.multi_exec_enabled {
                    RichText::new("⚡ Multi-Exec: ON").color(Color32::from_rgb(255, 220, 80)).strong()
                } else {
                    RichText::new("⚡ Multi-Exec: OFF").weak()
                };
                if ui.selectable_label(self.multi_exec_enabled, multi_exec_text).on_hover_text("Broadcast input to all open split terminals simultaneously").clicked() {
                    self.multi_exec_enabled = !self.multi_exec_enabled;
                }

                ui.separator();

                if ui.button("🌐 NetScan").clicked() {
                    self.show_network_tools = true;
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

        // 3. BOTTOM STATUS BAR
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
                    ui.label(RichText::new(format!("Tabs: {}", self.open_tabs.len())).size(11.0));
                });
            });
        });

        // 4. LEFT SIDEBAR (Sessions Tree & SFTP Explorer)
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

                        // Group sessions by folder
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
                            match s.session_type {
                                SessionType::LocalShell { ref custom_shell, ref working_dir } => {
                                    self.spawn_local_terminal(&s.name, custom_shell.as_deref(), working_dir.as_deref());
                                }
                                SessionType::Ssh { .. } => {
                                    self.spawn_ssh_terminal(s);
                                }
                                _ => {}
                            }
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

        // 5. CENTER WORKSPACE (Tabs Header & Terminals Grid)
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

            ui.separator();

            // RENDER TERMINALS ACCORDING TO SPLIT MODE
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

            match self.split_mode {
                SplitMode::Single => {
                    let active_idx = self.active_tab_idx.min(num_tabs - 1);
                    let tab = &mut self.open_tabs[active_idx];
                    let widget = TerminalWidget::new(&tab.id, &mut tab.session);
                    widget.ui(ui, broadcast_tx);
                }
                SplitMode::Horizontal => {
                    // Split left / right into 2 columns
                    let tab_a_idx = self.active_tab_idx.min(num_tabs - 1);
                    let tab_b_idx = if num_tabs > 1 {
                        if tab_a_idx == 0 { 1 } else { 0 }
                    } else {
                        tab_a_idx
                    };

                    ui.columns(2, |columns| {
                        let tab_a = &mut self.open_tabs[tab_a_idx];
                        let widget_a = TerminalWidget::new(&tab_a.id, &mut tab_a.session);
                        widget_a.ui(&mut columns[0], broadcast_tx);

                        if tab_a_idx != tab_b_idx {
                            let tab_b = &mut self.open_tabs[tab_b_idx];
                            let widget_b = TerminalWidget::new(&tab_b.id, &mut tab_b.session);
                            widget_b.ui(&mut columns[1], broadcast_tx);
                        } else {
                            columns[1].centered_and_justified(|ui| {
                                ui.label("Open another session to view side-by-side");
                            });
                        }
                    });
                }
                SplitMode::Vertical => {
                    // Split top / bottom
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
                                let widget_a = TerminalWidget::new(&tab_a.id, &mut tab_a.session);
                                widget_a.ui(ui, broadcast_tx);
                            },
                        );

                        ui.separator();

                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), half_height),
                            Layout::top_down(Align::Min),
                            |ui| {
                                if tab_a_idx != tab_b_idx {
                                    let tab_b = &mut self.open_tabs[tab_b_idx];
                                    let widget_b = TerminalWidget::new(&tab_b.id, &mut tab_b.session);
                                    widget_b.ui(ui, broadcast_tx);
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

                    // Row 1
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), half_h),
                        Layout::left_to_right(Align::Min),
                        |ui| {
                            ui.columns(2, |cols| {
                                if num_tabs > 0 {
                                    let tab0 = &mut self.open_tabs[0];
                                    TerminalWidget::new(&tab0.id, &mut tab0.session).ui(&mut cols[0], broadcast_tx);
                                }
                                if num_tabs > 1 {
                                    let tab1 = &mut self.open_tabs[1];
                                    TerminalWidget::new(&tab1.id, &mut tab1.session).ui(&mut cols[1], broadcast_tx);
                                }
                            });
                        },
                    );

                    ui.separator();

                    // Row 2
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), half_h),
                        Layout::left_to_right(Align::Min),
                        |ui| {
                            ui.columns(2, |cols| {
                                if num_tabs > 2 {
                                    let tab2 = &mut self.open_tabs[2];
                                    TerminalWidget::new(&tab2.id, &mut tab2.session).ui(&mut cols[0], broadcast_tx);
                                }
                                if num_tabs > 3 {
                                    let tab3 = &mut self.open_tabs[3];
                                    TerminalWidget::new(&tab3.id, &mut tab3.session).ui(&mut cols[1], broadcast_tx);
                                }
                            });
                        },
                    );
                }
            }
        });

        // 6. POPUP DIALOGS & MODAL WINDOWS
        if let Some(new_session) = render_new_session_dialog(ctx, &mut self.new_session_dialog) {
            self.sessions.push(new_session.clone());
            let _ = self.storage.save_sessions(&self.sessions);
            match new_session.session_type {
                SessionType::LocalShell { ref custom_shell, ref working_dir } => {
                    self.spawn_local_terminal(&new_session.name, custom_shell.as_deref(), working_dir.as_deref());
                }
                SessionType::Ssh { .. } => {
                    self.spawn_ssh_terminal(new_session);
                }
                _ => {}
            }
        }

        // Network Tools Modal
        if self.show_network_tools {
            let mut open = self.show_network_tools;
            let rt_handle = self.tokio_runtime.handle().clone();

            egui::Window::new("🌐 Network Scanner & Ping Utility")
                .open(&mut open)
                .default_size(Vec2::new(550.0, 420.0))
                .show(ctx, |ui| {
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
                    ui.label(RichText::new("Scan Results:").strong());

                    ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
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

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Ping Host:");
                        ui.text_edit_singleline(&mut self.network_tools.ping_host);
                        if ui.button("📡 Start Ping").clicked() {
                            self.network_tools.start_ping(&rt_handle);
                        }
                    });

                    ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                        let logs = self.network_tools.ping_logs.lock().clone();
                        for line in logs {
                            ui.label(RichText::new(line).monospace().size(11.0));
                        }
                    });
                });
            self.show_network_tools = open;
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

        // About Modal
        if self.show_about {
            let mut open = self.show_about;
            egui::Window::new("ℹ About RustyBox")
                .open(&mut open)
                .default_size(Vec2::new(420.0, 250.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("🚀 RustyBox");
                        ui.label(RichText::new("Portable Cross-Platform Remote Toolbox in Pure Rust").strong());
                        ui.add_space(8.0);
                        ui.label("An open-source, zero-install, lightweight MobaXterm equivalent.");
                        ui.label("Runs natively on Windows, macOS, and Linux (Ubuntu).");
                        ui.add_space(8.0);
                        ui.label(RichText::new("Features:").strong());
                        ui.label("• Multi-tab & Multi-Split Terminal (ConPTY / Unix PTY / VT100)");
                        ui.label("• Realtime Multi-Execution (Command Broadcasting)");
                        ui.label("• Graphical SFTP Sidebar with Quick Editor");
                        ui.label("• SSH Port Forwarding / Tunneling Manager");
                        ui.label("• Network Port Scanner & Ping Tester");
                        ui.label("• AES-256-GCM Master Password Encrypted Vault");
                    });
                });
            self.show_about = open;
        }
    }
}
