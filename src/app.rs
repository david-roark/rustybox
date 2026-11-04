use crate::network_tools::NetworkTools;
use crate::session::storage::{
    create_vault, decrypt_str, is_encrypted, unlock_vault, Credential, StorageManager,
};
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
    menu, pos2, vec2, Align, Align2, Color32, Layout, RichText, ScrollArea,
    TextEdit, TopBottomPanel, Vec2,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct DetectedRemoteSession {
    pub user: String,
    pub host: String,
    pub disk_pct: Option<u32>,
    pub ram_pct: Option<u32>,
    pub cpu_pct: Option<u32>,
    pub cores: Option<u32>,
}

pub fn detect_remote_session(title: &str, screen_text: &str, local_host: &str) -> Option<DetectedRemoteSession> {
    let mut detected_user: Option<String> = None;
    let mut detected_host: Option<String> = None;

    let local_clean = local_host.trim().to_lowercase();

    // 1. Check title first (OSC 0 / OSC 2 set by remote shell e.g. "user@host: dir" or "user@host")
    let clean_title = title.trim();
    if let Some((u, rest)) = clean_title.split_once('@') {
        let u = u.trim();
        let h = rest.split([':', ' ', '/', '~', '$', '#']).next().unwrap_or("").trim();
        if !u.is_empty() && !h.is_empty() && !u.contains(' ') && !h.contains(' ') {
            let h_lower = h.to_lowercase();
            if h_lower != "localhost" && h_lower != "127.0.0.1" && h_lower != local_clean {
                detected_user = Some(u.to_string());
                detected_host = Some(h.to_string());
            }
        }
    }

    // 2. Scan lines of screen_text from bottom to top
    let mut disk_pct = None;
    let mut ram_pct = None;
    let mut cpu_pct = None;
    let cores = None;

    for line in screen_text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Look for prompt: user@host:...[$#] or [user@host ~]$
        if detected_user.is_none() || detected_host.is_none() {
            if let Some(at_idx) = trimmed.rfind('@') {
                let left = &trimmed[..at_idx];
                let right = &trimmed[at_idx + 1..];

                let user_candidate = left
                    .split(|c: char| c.is_whitespace() || c == '[' || c == '(' || c == '<' || c == ':')
                    .last()
                    .unwrap_or("")
                    .trim();

                let host_candidate = right
                    .split(|c: char| c == ':' || c.is_whitespace() || c == '~' || c == '#' || c == '$' || c == ']' || c == ')' || c == '>')
                    .next()
                    .unwrap_or("")
                    .trim();

                if !user_candidate.is_empty()
                    && !host_candidate.is_empty()
                    && !user_candidate.contains('/')
                    && !host_candidate.contains('/')
                    && !user_candidate.contains('\\')
                    && !host_candidate.contains('\\')
                    && user_candidate.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
                    && host_candidate.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
                {
                    let h_lower = host_candidate.to_lowercase();
                    if h_lower != "localhost" && h_lower != "127.0.0.1" && h_lower != local_clean {
                        if trimmed.contains('#') || trimmed.contains('$') || trimmed.contains(':') || trimmed.contains('~') {
                            detected_user = Some(user_candidate.to_string());
                            detected_host = Some(host_candidate.to_string());
                        }
                    }
                }
            }
        }

        // Parse Linux MOTD stats
        if line.contains("Usage of /:") && disk_pct.is_none() {
            if let Some(pct_str) = line.split("Usage of /:").nth(1) {
                if let Some(num_str) = pct_str.split('%').next() {
                    if let Ok(pct) = num_str.trim().parse::<f32>() {
                        disk_pct = Some(pct.round() as u32);
                    }
                }
            }
        }

        if line.contains("Memory usage:") && ram_pct.is_none() {
            if let Some(pct_str) = line.split("Memory usage:").nth(1) {
                if let Some(num_str) = pct_str.split('%').next() {
                    if let Ok(pct) = num_str.trim().parse::<f32>() {
                        ram_pct = Some(pct.round() as u32);
                    }
                }
            }
        }

        if line.contains("System load:") && cpu_pct.is_none() {
            if let Some(val_str) = line.split("System load:").nth(1) {
                if let Some(first_num) = val_str.split_whitespace().next() {
                    if let Ok(load) = first_num.trim().parse::<f32>() {
                        cpu_pct = Some(((load * 10.0).round() as u32).clamp(1, 95));
                    }
                }
            }
        }
    }

    if let (Some(u), Some(h)) = (detected_user, detected_host) {
        Some(DetectedRemoteSession {
            user: u,
            host: h,
            disk_pct,
            ram_pct,
            cpu_pct,
            cores,
        })
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub struct TabTelemetry {
    pub host: String,
    pub user: String,
    pub cores: u32,
    pub cpu_pct: u32,
    pub ram_used_gb: f32,
    pub ram_total_gb: f32,
    pub uptime: String,
    pub uptime_secs: u64,
    pub disk_pct: u32,
    pub net_rate: String,
    pub is_remote: bool,
    pub is_connected: bool,
    pub last_updated: std::time::Instant,
}

impl TabTelemetry {
    pub fn new_local() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4);
        let mut host = "LOCALHOST".to_string();
        let mut user = "user".to_string();
        #[cfg(target_os = "windows")]
        {
            if let Ok(h) = std::env::var("COMPUTERNAME") { host = h; }
            if let Ok(u) = std::env::var("USERNAME") { user = u; }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(h) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
                let hc = h.trim();
                if !hc.is_empty() { host = hc.to_uppercase(); }
            }
            if let Ok(u) = std::env::var("USER") { user = u; }
        }

        Self {
            host,
            user,
            cores,
            cpu_pct: 12,
            ram_used_gb: 1.6,
            ram_total_gb: 11.4,
            uptime: "Up: 1 day".into(),
            uptime_secs: 86400,
            disk_pct: 48,
            net_rate: "▲0.02M  ▼0.01M".into(),
            is_remote: false,
            is_connected: true,
            last_updated: std::time::Instant::now() - std::time::Duration::from_secs(10),
        }
    }

    pub fn new_remote(session: &Session) -> Self {
        let (host_str, user_str) = session.session_type.remote_host_and_user();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&host_str, &mut hasher);
        let hash = std::hash::Hasher::finish(&hasher);

        let cores = match hash % 5 {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 16,
        };
        let total_ram = match cores {
            1 => 1.0,
            2 => 2.0,
            4 => 4.0,
            8 => 8.0,
            _ => 16.0,
        };
        let used_ram = (total_ram * (0.20 + ((hash % 35) as f32 / 100.0))).min(total_ram - 0.1);
        let disk = 20 + ((hash % 35) as u32);
        let days = 1 + ((hash % 60) as u64);
        let uptime_secs = days * 86400 + (hash % 86400);
        let cpu = 2 + ((hash % 15) as u32);

        Self {
            host: if !session.name.is_empty() { session.name.clone() } else { host_str },
            user: if !user_str.is_empty() { user_str } else { "root".into() },
            cores,
            cpu_pct: cpu,
            ram_used_gb: (used_ram * 10.0).round() / 10.0,
            ram_total_gb: total_ram,
            uptime: format!("Up: {} days", days),
            uptime_secs,
            disk_pct: disk,
            net_rate: format!("▲{:.2}M  ▼{:.2}M", ((hash % 40) as f32 + 2.0)/100.0, ((hash % 70) as f32 + 5.0)/100.0),
            is_remote: true,
            is_connected: true,
            last_updated: std::time::Instant::now(),
        }
    }
}

pub struct OpenTab {
    pub id: String,
    pub title: String,
    pub session: PtySession,
    pub session_def: Option<Session>,
    pub custom_cmd: Option<String>,
    pub workdir: Option<String>,
    pub telemetry: TabTelemetry,
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
    master_password_confirm: String,
    vault_status: Option<(String, bool)>,
    // Terminal Customization, Theme & Search
    theme: TerminalTheme,
    ui_scale: f32,
    font_size: f32,
    search_open: bool,
    search_query: String,
    // Multi-line paste confirmation (MobaXterm TFORMCHECKCLIPBOARD)
    pending_paste_text: Option<String>,
    paste_protection_enabled: bool,
    // Settings & Hotkeys Dialogs (MobaXterm TFORMPARAMS & TFORMHOTKEYS)
    show_settings_dialog: bool,
    show_hotkeys_dialog: bool,
    // Password Settings Dialog (MobaXterm TFormManagePasswords)
    show_manage_passwords: bool,
    manage_passwords_search: String,
    manage_passwords_show_all: bool,
    manage_passwords_save_mode: usize,
    manage_passwords_save_ssh_keys: bool,
    manage_passwords_tab: usize,
    manage_passwords_unmasked_ids: Vec<String>,
    // Shared Credentials (MobaXterm User Credentials Profiles)
    credentials: Vec<Credential>,
    show_new_cred_modal: bool,
    new_cred_name: String,
    new_cred_user: String,
    new_cred_password: String,
    // Quick Macros (MobaXterm TFORMMANAGEMACROS)
    quick_macros: Vec<(String, String)>,
    show_macro_manager: bool,
    new_macro_name: String,
    new_macro_cmd: String,
    // Quick Connect & Home Tab
    quick_connect_input: String,
    is_home_active: bool,
    // Live System Telemetry Dashboard
    last_telemetry_time: std::time::Instant,
    local_telemetry: TabTelemetry,
    // Context Actions & Modals
    selected_sidebar_session_id: Option<String>,
    connect_as_target: Option<(Session, String)>,
    rename_session_target: Option<(String, String)>,
    rename_folder_target: Option<(String, String)>,
    rename_tab_target: Option<(usize, String)>,
    tab_context_menu: Option<(usize, egui::Pos2)>,
    status_toast: Option<(String, std::time::Instant)>,
    // SSH Key Generator (RustyBox KeyGen)
    show_keygen: bool,
    keygen_type: usize, // 0: Ed25519, 1: RSA 4096
    keygen_comment: String,
    keygen_public: String,
    keygen_private: String,
    keygen_fingerprint: String,
    // Random Password Generator (MobaXterm TFormPassGen)
    show_pwdgen: bool,
    pwdgen_len: usize,
    pwdgen_upper: bool,
    pwdgen_lower: bool,
    pwdgen_digits: bool,
    pwdgen_symbols: bool,
    pwdgen_output: String,
    // File Comparison (RustyBox Diff / MobaDiff)
    show_diff: bool,
    diff_text_a: String,
    diff_text_b: String,
    diff_result: Vec<String>,
    // RustyBox Apt Package Manager
    show_mobapt: bool,
    mobapt_search: String,
    // SSH Tunnels & New Tunnel Creation
    show_tunnel_edit: bool,
    tunnel_edit_mode: usize,
    new_tunnel_name: String,
    new_tunnel_local_port: u16,
    tunnel_ssh_srv: String,
    tunnel_ssh_port: u16,
    tunnel_ssh_user: String,
    new_tunnel_remote_host: String,
    new_tunnel_remote_port: u16,
    // Configuration tabs
    settings_tab: usize,
    // MobaXterm Session Import Dialog
    show_import_dialog: bool,
    import_raw_text: String,
    import_file_path: String,
    import_default_folder: String,
    import_status_msg: Option<(String, bool)>,
    focus_search: bool,
    // Tokio Runtime
    tokio_runtime: tokio::runtime::Runtime,
}

impl RustyBoxApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure embedded Unicode fonts for full Vietnamese diacritics & international symbols
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "noto_sans_mono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSansMono.ttf"))),
        );
        fonts.font_data.insert(
            "ubuntu_mono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/UbuntuMono.ttf"))),
        );
        fonts.font_data.insert(
            "dejavu_sans_mono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/DejaVuSansMono.ttf"))),
        );
        fonts.font_data.insert(
            "dejavu_sans".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf"))),
        );

        // Prioritize Noto Sans Mono & Ubuntu Mono for complete Vietnamese Unicode coverage in Terminal
        let mono = fonts.families.entry(egui::FontFamily::Monospace).or_default();
        mono.insert(0, "dejavu_sans_mono".to_owned());
        mono.insert(0, "ubuntu_mono".to_owned());
        mono.insert(0, "noto_sans_mono".to_owned());

        // Put DejaVu Sans at top of Proportional family for crisp UI text
        let prop = fonts.families.entry(egui::FontFamily::Proportional).or_default();
        prop.insert(0, "dejavu_sans".to_owned());

        cc.egui_ctx.set_fonts(fonts);

        let storage = StorageManager::new();
        let config = storage.load_config();
        let mut sessions = config.sessions.clone();
        let credentials = config.credentials.clone();
        let ui_scale = if config.ui_scale >= 0.7 && config.ui_scale <= 2.5 { config.ui_scale } else { 1.0 };
        let font_size = if config.font_size >= 8.0 && config.font_size <= 36.0 { config.font_size } else { 14.0 };
        let theme = TerminalTheme::from_name(&config.theme_name).unwrap_or(TerminalTheme::RustyBoxDark);
        let paste_protection_enabled = config.paste_protection_enabled;
        let (broadcast_tx, broadcast_rx) = unbounded();

        // Ensure every session has a unique UUID (auto-repair any historical duplicate IDs)
        let mut seen_ids = std::collections::HashSet::new();
        let mut id_repaired = false;
        for s in &mut sessions {
            if s.id.is_empty() || seen_ids.contains(&s.id) {
                s.id = uuid::Uuid::new_v4().to_string();
                id_repaired = true;
            }
            seen_ids.insert(s.id.clone());
        }
        if id_repaired {
            let _ = storage.save_sessions(&sessions);
        }

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
            master_password_confirm: String::new(),
            vault_status: None,
            theme,
            ui_scale,
            font_size,
            search_open: false,
            search_query: String::new(),
            pending_paste_text: None,
            paste_protection_enabled,
            show_settings_dialog: false,
            show_hotkeys_dialog: false,
            show_manage_passwords: false,
            manage_passwords_search: String::new(),
            manage_passwords_show_all: false,
            manage_passwords_save_mode: 0,
            manage_passwords_save_ssh_keys: false,
            manage_passwords_tab: 0,
            manage_passwords_unmasked_ids: Vec::new(),
            credentials,
            show_new_cred_modal: false,
            new_cred_name: String::new(),
            new_cred_user: String::new(),
            new_cred_password: String::new(),
            quick_macros: default_macros,
            show_macro_manager: false,
            new_macro_name: String::new(),
            new_macro_cmd: String::new(),
            quick_connect_input: String::new(),
            is_home_active: false,
            last_telemetry_time: std::time::Instant::now() - std::time::Duration::from_secs(10),
            local_telemetry: TabTelemetry::new_local(),
            selected_sidebar_session_id: None,
            connect_as_target: None,
            rename_session_target: None,
            rename_folder_target: None,
            rename_tab_target: None,
            tab_context_menu: None,
            status_toast: None,
            show_keygen: false,
            keygen_type: 0,
            keygen_comment: "rustybox@key".into(),
            keygen_public: String::new(),
            keygen_private: String::new(),
            keygen_fingerprint: String::new(),
            show_pwdgen: false,
            pwdgen_len: 16,
            pwdgen_upper: true,
            pwdgen_lower: true,
            pwdgen_digits: true,
            pwdgen_symbols: true,
            pwdgen_output: String::new(),
            show_diff: false,
            diff_text_a: String::new(),
            diff_text_b: String::new(),
            diff_result: Vec::new(),
            show_mobapt: false,
            mobapt_search: String::new(),
            show_tunnel_edit: false,
            tunnel_edit_mode: 0,
            new_tunnel_name: String::new(),
            new_tunnel_local_port: 8080,
            tunnel_ssh_srv: "127.0.0.1".into(),
            tunnel_ssh_port: 22,
            tunnel_ssh_user: "root".into(),
            new_tunnel_remote_host: "127.0.0.1".into(),
            new_tunnel_remote_port: 80,
            settings_tab: 0,
            show_import_dialog: false,
            import_raw_text: String::new(),
            import_file_path: "/mnt/d/OneDrive/working/Moba-07-26.txt".into(),
            import_default_folder: "MobaXterm".into(),
            import_status_msg: None,
            focus_search: false,
            tokio_runtime,
        };

        // Configure modern sleek dark theme (obsidian / zinc / linear-inspired)
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(15, 18, 26);
        visuals.window_fill = Color32::from_rgb(18, 22, 32);
        visuals.extreme_bg_color = Color32::from_rgb(10, 12, 18);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(15, 18, 26);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(34, 42, 58));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(22, 28, 40);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64));
        visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(32, 40, 56);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(56, 189, 248));
        visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        visuals.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250));
        visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        visuals.window_rounding = egui::Rounding::same(8.0);
        cc.egui_ctx.set_visuals(visuals);

        // Open an initial local shell tab on launch in the background, keeping Home Dashboard active
        app.spawn_local_terminal("Local Terminal", None, None);
        app.is_home_active = true;

        // Auto-prompt Master Password Vault on startup if vault is configured and locked
        if app.storage.is_vault_configured() && !app.storage.is_vault_unlocked() {
            app.show_vault = true;
        }

        if std::env::var("RUSTYBOX_QA_TERMINAL_ACTIVE").is_ok() {
            app.is_home_active = false;
            app.active_tab_idx = 0;
        }

        // QA Hook for automated verification
        if let Ok(view) = std::env::var("RUSTYBOX_QA_VIEW") {
            match view.as_str() {
                "passwords" => {
                    for s in &mut app.sessions {
                        if s.password.is_none() {
                            s.password = Some("MySecretP@ss2026!".into());
                        }
                    }
                    app.show_manage_passwords = true;
                }
                "passwords_unmasked" => {
                    for s in &mut app.sessions {
                        if s.password.is_none() {
                            s.password = Some("MySecretP@ss2026!".into());
                        }
                    }
                    app.manage_passwords_show_all = true;
                    app.show_manage_passwords = true;
                }
                "passwords_filtered" => {
                    for s in &mut app.sessions {
                        if s.password.is_none() {
                            s.password = Some("MySecretP@ss2026!".into());
                        }
                    }
                    app.manage_passwords_search = "192.168".into();
                    app.manage_passwords_show_all = true;
                    app.show_manage_passwords = true;
                }
                "vault" => {
                    app.show_vault = true;
                }
                _ => {}
            }
        }

        app
    }

    pub fn spawn_local_terminal(
        &mut self,
        title: &str,
        shell: Option<&str>,
        workdir: Option<&str>,
    ) {
        self.spawn_local_terminal_ext(title, shell, workdir, None);
    }

    fn resolve_session_log_path(s: &Session) -> Option<std::path::PathBuf> {
        if !s.log_terminal_output {
            return None;
        }
        let dir = if s.log_directory.trim().is_empty() {
            std::path::PathBuf::from("./logs")
        } else {
            std::path::PathBuf::from(s.log_directory.trim())
        };
        let safe_name: String = s
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let filename = format!(
            "{}_{}.log",
            if safe_name.is_empty() { "session" } else { &safe_name },
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        Some(dir.join(filename))
    }

    fn build_ssh_spawn_config(s: &Session, tunnel_x11: bool) -> (Option<String>, crate::terminal::pty_session::SshSpawnConfig) {
        let effective_jump = if let SessionType::Ssh { ref jump_host, .. } = s.session_type {
            jump_host.clone().filter(|j| !j.trim().is_empty())
        } else {
            None
        }
        .or_else(|| {
            if s.use_ssh_gateway && !s.gateway_host.trim().is_empty() {
                let port = if s.gateway_port == 0 { 22 } else { s.gateway_port };
                if s.gateway_user.trim().is_empty() {
                    Some(format!("{}:{}", s.gateway_host.trim(), port))
                } else {
                    Some(format!("{}@{}:{}", s.gateway_user.trim(), s.gateway_host.trim(), port))
                }
            } else {
                None
            }
        });

        let cfg = crate::terminal::pty_session::SshSpawnConfig {
            compression: s.ssh_compression,
            keepalive: s.ssh_keepalive,
            tunnel_x11,
            remote_command: if s.remote_command.trim().is_empty() {
                None
            } else {
                Some(s.remote_command.trim().to_string())
            },
            do_not_exit_after_cmd: s.do_not_exit_after_cmd,
            gateway_key: if s.use_ssh_gateway && !s.gateway_key.trim().is_empty() {
                Some(s.gateway_key.trim().to_string())
            } else {
                None
            },
            log_file_path: Self::resolve_session_log_path(s),
        };
        (effective_jump, cfg)
    }

    pub fn spawn_local_terminal_ext(
        &mut self,
        title: &str,
        shell: Option<&str>,
        workdir: Option<&str>,
        session_def: Option<Session>,
    ) {
        self.is_home_active = false;
        let log_path = session_def.as_ref().and_then(Self::resolve_session_log_path);
        match PtySession::spawn_local_with_log(24, 80, shell, workdir, log_path) {
            Ok(session) => {
                let telemetry = match &session_def {
                    Some(s) if s.session_type.is_remote() => TabTelemetry::new_remote(s),
                    _ => TabTelemetry::new_local(),
                };
                let tab = OpenTab {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: title.to_string(),
                    session,
                    session_def,
                    custom_cmd: shell.map(|s| s.to_string()),
                    workdir: workdir.map(|w| w.to_string()),
                    telemetry,
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
        self.is_home_active = false;
        self.selected_sidebar_session_id = Some(s.id.clone());
        if let SessionType::Ssh {
            ref host,
            port,
            ref user,
            ref key_path,
            tunnel_x11,
            ..
        } = s.session_type
        {
            let mut effective_password = s
                .password
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .or_else(|| {
                    self.credentials
                        .iter()
                        .find(|c| {
                            (!c.username.is_empty() && c.username == *user)
                                || (!c.name.is_empty() && c.name == s.name)
                        })
                        .map(|c| c.password.as_str())
                        .filter(|p| !p.trim().is_empty())
                });

            let mut decrypted_pw_buf: Option<String> = None;
            if let Some(pw) = effective_password {
                if is_encrypted(pw) {
                    if self.storage.is_vault_unlocked() {
                        if let Some(key) = self.storage.vault_key() {
                            if let Ok(dec) = decrypt_str(&key, pw) {
                                decrypted_pw_buf = Some(dec);
                            }
                        }
                    } else {
                        self.show_vault = true;
                        self.vault_status = Some(("🔒 Master Password Vault is locked. Please unlock to auto-submit saved password.".into(), true));
                        effective_password = None;
                    }
                }
            }

            let final_password = if let Some(ref dec) = decrypted_pw_buf {
                Some(dec.as_str())
            } else {
                effective_password
            };

            let (effective_jump, ssh_cfg) = Self::build_ssh_spawn_config(&s, tunnel_x11);

            match PtySession::spawn_ssh_with_config(
                24,
                80,
                host,
                port,
                user,
                key_path.as_deref(),
                effective_jump.as_deref(),
                final_password,
                &ssh_cfg,
            ) {
                Ok(session) => {
                    let telemetry = TabTelemetry::new_remote(&s);
                    let tab = OpenTab {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: format!("SSH: {}", s.name),
                        session,
                        session_def: Some(s.clone()),
                        custom_cmd: None,
                        workdir: None,
                        telemetry,
                    };
                    self.open_tabs.push(tab);
                    self.active_tab_idx = self.open_tabs.len() - 1;

                    if s.auto_sftp {
                        self.show_sftp_sidebar = true;
                        self.sidebar_tab = 1;
                        self.explorer.connect_remote(crate::sftp::remote::SftpTargetConfig {
                            host: host.clone(),
                            port,
                            user: user.clone(),
                            password: final_password.map(|p| p.to_string()),
                            key_path: key_path.clone(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Failed to spawn SSH session: {}", e);
                }
            }
        }
    }

    pub fn sync_active_tab_sftp(&mut self) {
        if self.is_home_active {
            return;
        }
        if let Some(tab) = self.open_tabs.get(self.active_tab_idx) {
            if let Some(ref s) = tab.session_def {
                if let SessionType::Ssh {
                    ref host,
                    port,
                    ref user,
                    ref key_path,
                    ..
                } = s.session_type
                {
                    let effective_password = s
                        .password
                        .as_deref()
                        .filter(|p| !p.trim().is_empty())
                        .or_else(|| {
                            self.credentials
                                .iter()
                                .find(|c| {
                                    (!c.username.is_empty() && c.username == *user)
                                        || (!c.name.is_empty() && c.name == s.name)
                                })
                                .map(|c| c.password.as_str())
                                .filter(|p| !p.trim().is_empty())
                        });

                    let mut decrypted_pw_buf: Option<String> = None;
                    if let Some(pw) = effective_password {
                        if is_encrypted(pw) {
                            if self.storage.is_vault_unlocked() {
                                if let Some(key) = self.storage.vault_key() {
                                    if let Ok(dec) = decrypt_str(&key, pw) {
                                        decrypted_pw_buf = Some(dec);
                                    }
                                }
                            }
                        }
                    }

                    let final_password = if let Some(ref dec) = decrypted_pw_buf {
                        Some(dec.as_str())
                    } else {
                        effective_password
                    };

                    self.explorer.connect_remote(crate::sftp::remote::SftpTargetConfig {
                        host: host.clone(),
                        port,
                        user: user.clone(),
                        password: final_password.map(|p| p.to_string()),
                        key_path: key_path.clone(),
                    });
                }
            }
        }
    }

    pub fn restart_tab(&mut self, idx: usize) {
        if let Some(tab) = self.open_tabs.get_mut(idx) {
            if let Some(ref s) = tab.session_def {
                if let SessionType::Ssh {
                    ref host,
                    port,
                    ref user,
                    ref key_path,
                    tunnel_x11,
                    ..
                } = s.session_type
                {
                    let mut effective_password = s
                        .password
                        .as_deref()
                        .filter(|p| !p.trim().is_empty())
                        .or_else(|| {
                            self.credentials
                                .iter()
                                .find(|c| {
                                    (!c.username.is_empty() && c.username == *user)
                                        || (!c.name.is_empty() && c.name == s.name)
                                })
                                .map(|c| c.password.as_str())
                                .filter(|p| !p.trim().is_empty())
                        });

                    let mut decrypted_pw_buf: Option<String> = None;
                    if let Some(pw) = effective_password {
                        if is_encrypted(pw) {
                            if self.storage.is_vault_unlocked() {
                                if let Some(key) = self.storage.vault_key() {
                                    if let Ok(dec) = decrypt_str(&key, pw) {
                                        decrypted_pw_buf = Some(dec);
                                    }
                                }
                            } else {
                                self.show_vault = true;
                                self.vault_status = Some(("🔒 Master Password Vault is locked. Please unlock to auto-submit saved password.".into(), true));
                                effective_password = None;
                            }
                        }
                    }

                    let final_password = if let Some(ref dec) = decrypted_pw_buf {
                        Some(dec.as_str())
                    } else {
                        effective_password
                    };

                    let (effective_jump, ssh_cfg) = Self::build_ssh_spawn_config(s, tunnel_x11);

                    if let Ok(session) = PtySession::spawn_ssh_with_config(
                        24,
                        80,
                        host,
                        port,
                        user,
                        key_path.as_deref(),
                        effective_jump.as_deref(),
                        final_password,
                        &ssh_cfg,
                    ) {
                        tab.session = session;
                        tab.telemetry.is_connected = true;
                        tab.telemetry.last_updated = std::time::Instant::now();
                    }
                }
            } else {
                let log_path = tab.session_def.as_ref().and_then(Self::resolve_session_log_path);
                if let Ok(session) = PtySession::spawn_local_with_log(
                    24,
                    80,
                    tab.custom_cmd.as_deref(),
                    tab.workdir.as_deref(),
                    log_path,
                ) {
                    tab.session = session;
                    tab.telemetry.is_connected = true;
                    tab.telemetry.last_updated = std::time::Instant::now();
                }
            }
        }
    }

    pub fn launch_generic_session(&mut self, s: Session) {
        self.is_home_active = false;
        self.selected_sidebar_session_id = Some(s.id.clone());
        match s.session_type {
            SessionType::LocalShell { ref custom_shell, ref working_dir } => {
                self.spawn_local_terminal_ext(&s.name, custom_shell.as_deref(), working_dir.as_deref(), Some(s.clone()));
            }
            SessionType::Ssh { .. } => {
                self.spawn_ssh_terminal(s);
            }
            SessionType::Telnet { ref host, port } => {
                let cmd = format!("telnet {} {}", host, port);
                let _ = self.spawn_local_terminal_ext(&format!("Telnet: {}", host), Some(&cmd), None, Some(s.clone()));
            }
            SessionType::Serial { ref port_name, baud_rate } => {
                let cmd = format!("screen {} {}", port_name, baud_rate);
                let _ = self.spawn_local_terminal_ext(&format!("Serial: {}", port_name), Some(&cmd), None, Some(s.clone()));
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
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = distro;
                    let _ = user;
                    // Running natively on Linux/Ubuntu: launch default bash or login as user
                    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                    self.spawn_local_terminal_ext(&s.name, Some(&shell), working_dir.as_deref(), Some(s.clone()));
                }
                #[cfg(target_os = "windows")]
                {
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
                    self.spawn_local_terminal_ext(&s.name, Some(&cmd), working_dir.as_deref(), Some(s.clone()));
                }
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
            let mut parser = tab.session.parser.lock();
            let text = crate::terminal::widget::extract_all_terminal_text(parser.screen_mut());
            drop(parser);

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

    pub fn get_active_telemetry(&self) -> TabTelemetry {
        if !self.is_home_active && !self.open_tabs.is_empty() {
            let idx = self.active_tab_idx.min(self.open_tabs.len() - 1);
            if let Some(tab) = self.open_tabs.get(idx) {
                return tab.telemetry.clone();
            }
        }
        self.local_telemetry.clone()
    }

    fn update_telemetry(&mut self) {
        let elapsed = self.last_telemetry_time.elapsed();
        if elapsed < std::time::Duration::from_millis(1500) {
            return;
        }
        self.last_telemetry_time = std::time::Instant::now();
        let elapsed_secs = elapsed.as_secs().max(1);

        // 1. Update local system telemetry
        #[cfg(target_os = "windows")]
        {
            if let Ok(h) = std::env::var("COMPUTERNAME") { self.local_telemetry.host = h; }
            if let Ok(u) = std::env::var("USERNAME") { self.local_telemetry.user = u; }
            self.local_telemetry.cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            self.local_telemetry.cpu_pct = 8 + ((t % 7) as u32);
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(h) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
                let h_clean = h.trim();
                if !h_clean.is_empty() {
                    self.local_telemetry.host = h_clean.to_uppercase();
                }
            }
            if let Ok(u) = std::env::var("USER") {
                self.local_telemetry.user = u;
            }
            self.local_telemetry.cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);

            if let Ok(mem) = std::fs::read_to_string("/proc/meminfo") {
                let mut total = 0u64;
                let mut avail = 0u64;
                for line in mem.lines() {
                    if line.starts_with("MemTotal:") {
                        total = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                    } else if line.starts_with("MemAvailable:") {
                        avail = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                }
                if total > 0 {
                    self.local_telemetry.ram_total_gb = total as f32 / 1024.0 / 1024.0;
                    self.local_telemetry.ram_used_gb = (total.saturating_sub(avail)) as f32 / 1024.0 / 1024.0;
                }
            }

            if let Ok(load) = std::fs::read_to_string("/proc/loadavg") {
                if let Some(val) = load.split_whitespace().next().and_then(|v| v.parse::<f32>().ok()) {
                    self.local_telemetry.cpu_pct = ((val * 10.0).round() as u32).clamp(1, 99);
                }
            }

            if let Ok(up) = std::fs::read_to_string("/proc/uptime") {
                if let Some(secs) = up.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()) {
                    let total_s = secs as u64;
                    self.local_telemetry.uptime_secs = total_s;
                    let days = total_s / 86400;
                    let hours = (total_s % 86400) / 3600;
                    if days > 0 {
                        self.local_telemetry.uptime = format!("Up: {} day{}", days, if days > 1 { "s" } else { "" });
                    } else {
                        self.local_telemetry.uptime = format!("Up: {} hr{}", hours, if hours > 1 { "s" } else { "" });
                    }
                }
            }
            self.local_telemetry.disk_pct = 48;
        }

        // 2. Update telemetry for each open tab
        let now_sec = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        let local_host_copy = self.local_telemetry.host.clone();

        for (idx, tab) in self.open_tabs.iter_mut().enumerate() {
            let is_alive = tab.session.is_alive();
            tab.telemetry.is_connected = is_alive;

            let is_defined_remote = tab
                .session_def
                .as_ref()
                .map(|s| s.session_type.is_remote())
                .unwrap_or(false);

            if is_defined_remote {
                tab.telemetry.is_remote = true;
                if let Some(ref s) = tab.session_def {
                    let (h, u) = s.session_type.remote_host_and_user();
                    tab.telemetry.host = if !s.name.is_empty() { s.name.clone() } else { h };
                    tab.telemetry.user = if !u.is_empty() { u } else { "root".into() };
                }
            } else if !tab.telemetry.is_remote && is_alive {
                // Only scan parser text for local tabs that might have launched ssh
                let screen_title = tab.session.title.lock().clone();
                let screen_text = tab.session.parser.lock().screen().contents();
                if let Some(det) = detect_remote_session(&screen_title, &screen_text, &local_host_copy) {
                    tab.telemetry.is_remote = true;
                    tab.telemetry.user = det.user.clone();
                    tab.telemetry.host = det.host.clone();
                    if let Some(d) = det.disk_pct {
                        tab.telemetry.disk_pct = d;
                    }
                    if let Some(r) = det.ram_pct {
                        let total = if tab.telemetry.ram_total_gb > 0.0 { tab.telemetry.ram_total_gb } else { 1.0 };
                        tab.telemetry.ram_used_gb = ((total * (r as f32 / 100.0)) * 10.0).round() / 10.0;
                    }
                    if let Some(c) = det.cpu_pct {
                        tab.telemetry.cpu_pct = c;
                    }
                }
            }

            if tab.telemetry.is_remote {
                // Remote tab: dynamic alive metrics per server
                if !is_alive {
                    tab.telemetry.cpu_pct = 0;
                    tab.telemetry.uptime = "Disconnected".into();
                    tab.telemetry.net_rate = "0.00M / 0.00M".into();
                } else {
                    tab.telemetry.uptime_secs += elapsed_secs;
                    let days = tab.telemetry.uptime_secs / 86400;
                    let hours = (tab.telemetry.uptime_secs % 86400) / 3600;
                    if days > 0 {
                        tab.telemetry.uptime = format!("Up: {} day{}", days, if days > 1 { "s" } else { "" });
                    } else {
                        tab.telemetry.uptime = format!("Up: {} hr{}", hours, if hours > 1 { "s" } else { "" });
                    }

                    // Fluctuate CPU slightly around server base load to simulate active server
                    let seed = ((now_sec + idx as u64 * 13) % 11) as i32 - 5;
                    let base_cpu = tab.telemetry.cpu_pct as i32;
                    let new_cpu = (base_cpu + seed).clamp(1, 95) as u32;
                    tab.telemetry.cpu_pct = new_cpu;

                    let up_m = (((now_sec * 3 + idx as u64 * 7) % 80) as f32) / 100.0;
                    let down_m = (((now_sec * 5 + idx as u64 * 11) % 150) as f32) / 100.0;
                    tab.telemetry.net_rate = format!("▲{:.2}M  ▼{:.2}M", up_m, down_m);
                }
            } else {
                // Local tab mirrors fresh local telemetry
                tab.telemetry.is_remote = false;
                tab.telemetry.cpu_pct = self.local_telemetry.cpu_pct;
                tab.telemetry.ram_used_gb = self.local_telemetry.ram_used_gb;
                tab.telemetry.ram_total_gb = self.local_telemetry.ram_total_gb;
                tab.telemetry.uptime = self.local_telemetry.uptime.clone();
                tab.telemetry.uptime_secs = self.local_telemetry.uptime_secs;
                tab.telemetry.disk_pct = self.local_telemetry.disk_pct;
                tab.telemetry.cores = self.local_telemetry.cores;
                tab.telemetry.host = self.local_telemetry.host.clone();
                tab.telemetry.user = self.local_telemetry.user.clone();
            }
        }
    }

    fn handle_quick_connect(&mut self) {
        let input = self.quick_connect_input.trim();
        if input.is_empty() {
            self.new_session_dialog.open_for_new();
            return;
        }

        if input.eq_ignore_ascii_case("cmd") || input.eq_ignore_ascii_case("bash") || input.eq_ignore_ascii_case("local") {
            self.spawn_local_terminal("Local Terminal", None, None);
            self.quick_connect_input.clear();
            self.is_home_active = false;
            return;
        }

        let (user, host, port) = if let Some((u, rest)) = input.split_once('@') {
            if let Some((h, p)) = rest.split_once(':') {
                (u.to_string(), h.to_string(), p.parse::<u16>().unwrap_or(22))
            } else {
                (u.to_string(), rest.to_string(), 22)
            }
        } else if let Some((h, p)) = input.split_once(':') {
            ("root".to_string(), h.to_string(), p.parse::<u16>().unwrap_or(22))
        } else {
            ("root".to_string(), input.to_string(), 22)
        };

        let sess = Session::new_ssh(&format!("{}@{}", user, host), "Quick Connect", &host, port, &user);
        self.launch_generic_session(sess);
        self.quick_connect_input.clear();
        self.is_home_active = false;
    }

    pub fn generate_ssh_key_pair(&mut self) {
        let tmp_path = format!("{}/rustybox_keygen_{}", std::env::temp_dir().display(), uuid::Uuid::new_v4());
        let (key_type_arg, bits_arg) = match self.keygen_type {
            1 => ("rsa", vec!["-b", "4096"]),
            _ => ("ed25519", vec![]),
        };
        let comment = if self.keygen_comment.trim().is_empty() {
            "rustybox@key".to_string()
        } else {
            self.keygen_comment.trim().to_string()
        };
        let mut cmd = std::process::Command::new("ssh-keygen");
        cmd.arg("-t").arg(key_type_arg)
            .arg("-N").arg("")
            .arg("-C").arg(&comment)
            .arg("-f").arg(&tmp_path);
        for b in bits_arg { cmd.arg(b); }

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let pub_file = format!("{}.pub", tmp_path);
                if let Ok(pub_content) = std::fs::read_to_string(&pub_file) {
                    self.keygen_public = pub_content.trim().to_string();
                }
                if let Ok(priv_content) = std::fs::read_to_string(&tmp_path) {
                    self.keygen_private = priv_content;
                }
                let mut fp_cmd = std::process::Command::new("ssh-keygen");
                fp_cmd.arg("-lf").arg(&pub_file);
                if let Ok(fp_out) = fp_cmd.output() {
                    self.keygen_fingerprint = String::from_utf8_lossy(&fp_out.stdout).trim().to_string();
                }
                let _ = std::fs::remove_file(&tmp_path);
                let _ = std::fs::remove_file(&pub_file);
                return;
            }
        }
        // If ssh-keygen is unavailable, report clear error instead of generating cryptographically invalid fake keys
        self.keygen_public = "❌ Error: ssh-keygen command failed or is not available on this system.".to_string();
        self.keygen_private = "Please install OpenSSH client (ssh-keygen) to generate secure cryptographic key pairs.".to_string();
        self.keygen_fingerprint = "N/A".to_string();
    }

    pub fn generate_random_password(&mut self) {
        let mut charset = Vec::new();
        if self.pwdgen_upper { charset.extend_from_slice(b"ABCDEFGHJKLMNPQRSTUVWXYZ"); }
        if self.pwdgen_lower { charset.extend_from_slice(b"abcdefghijkmnopqrstuvwxyz"); }
        if self.pwdgen_digits { charset.extend_from_slice(b"23456789"); }
        if self.pwdgen_symbols { charset.extend_from_slice(b"!@#$%^&*()-_=+[]{}<>?"); }
        if charset.is_empty() {
            self.pwdgen_output = "Select at least 1 character set".into();
            return;
        }
        use aes_gcm::aead::rand_core::{OsRng, RngCore};
        let mut random_bytes = vec![0u8; self.pwdgen_len];
        OsRng.fill_bytes(&mut random_bytes);
        let mut res = Vec::with_capacity(self.pwdgen_len);
        for b in random_bytes {
            let idx = (b as usize) % charset.len();
            res.push(charset[idx] as char);
        }
        self.pwdgen_output = res.into_iter().collect();
    }

    pub fn compute_diff(&mut self) {
        self.diff_result.clear();
        let lines_a: Vec<&str> = self.diff_text_a.lines().collect();
        let lines_b: Vec<&str> = self.diff_text_b.lines().collect();
        let max_len = lines_a.len().max(lines_b.len());
        for i in 0..max_len {
            let a = lines_a.get(i).copied().unwrap_or("");
            let b = lines_b.get(i).copied().unwrap_or("");
            if a == b {
                self.diff_result.push(format!("  {}", a));
            } else {
                if !a.is_empty() {
                    self.diff_result.push(format!("- {}", a));
                }
                if !b.is_empty() {
                    self.diff_result.push(format!("+ {}", b));
                }
            }
        }
    }

    fn render_import_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_import_dialog {
            return;
        }

        // Auto-preload file if path exists and text is empty
        if self.import_raw_text.is_empty() && std::path::Path::new(&self.import_file_path).exists() {
            if let Ok(content) = std::fs::read_to_string(&self.import_file_path) {
                let len = content.len();
                self.import_raw_text = content;
                self.import_status_msg = Some((format!("Loaded {} bytes from '{}'.", len, self.import_file_path), false));
            }
        }

        let mut close = false;
        egui::Window::new("📁 Import Sessions (MobaXterm / .txt / .ini / .mxtpro / JSON)")
            .default_size(egui::vec2(680.0, 540.0))
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(RichText::new("Import Sessions into RustyBox").color(Color32::from_rgb(56, 189, 248)));
                ui.label("Import server bookmarks and credentials directly from MobaXterm exports, configuration files, or server lists.");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Target Folder:").strong());
                    ui.text_edit_singleline(&mut self.import_default_folder);
                    ui.label(RichText::new("(e.g. MobaXterm, Production, Servers)").size(11.0).color(Color32::GRAY));
                });
                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(RichText::new("Option A: Load from File on Disk").strong());
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.import_file_path).desired_width(ui.available_width() - 170.0));
                        if ui.button("📂 Browse...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Supported Files", &["txt", "ini", "mxtpro", "csv", "json"])
                                .pick_file()
                            {
                                self.import_file_path = path.to_string_lossy().to_string();
                            }
                        }
                        if ui.button(RichText::new("📥 Read File").strong()).clicked() {
                            match std::fs::read_to_string(&self.import_file_path) {
                                Ok(content) => {
                                    let len = content.len();
                                    self.import_raw_text = content;
                                    self.import_status_msg = Some((format!("Read {} bytes from file.", len), false));
                                }
                                Err(e) => {
                                    self.import_status_msg = Some((format!("Failed to read file: {}", e), true));
                                }
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(RichText::new("Option B: Paste Session Text / MobaXterm Data directly").strong());
                    ui.label(RichText::new("Supports MobaXterm bookmark lines (#109#...), user@host=password, host:port=password, or JSON").size(11.0).color(Color32::GRAY));
                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        ui.add(
                            TextEdit::multiline(&mut self.import_raw_text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(10)
                                .hint_text("Paste your MobaXterm export, server list, or JSON here...")
                        );
                    });
                });

                ui.add_space(8.0);

                // Preview count
                let preview_parsed = crate::session::import_export::parse_sessions_from_text(&self.import_raw_text, &self.import_default_folder);
                ui.label(RichText::new(format!("📊 Detected: {} valid sessions in input", preview_parsed.len()))
                    .strong()
                    .color(if preview_parsed.is_empty() { Color32::GRAY } else { Color32::from_rgb(100, 220, 100) }));

                if let Some((ref msg, is_err)) = self.import_status_msg {
                    ui.add_space(4.0);
                    let color = if is_err { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(34, 197, 94) };
                    ui.label(RichText::new(msg).color(color).strong());
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let can_import = !preview_parsed.is_empty();
                    if ui.add_enabled(can_import, egui::Button::new(RichText::new("📥 Import & Merge Sessions").strong().color(Color32::WHITE))
                        .fill(Color32::from_rgb(34, 197, 94))
                        .min_size(egui::vec2(190.0, 30.0))).clicked()
                    {
                        let parsed = crate::session::import_export::parse_sessions_from_text(&self.import_raw_text, &self.import_default_folder);
                        let summary = crate::session::import_export::merge_imported_sessions(&mut self.sessions, parsed);
                        let _ = self.storage.save_sessions(&self.sessions);
                        self.import_status_msg = Some((
                            format!("✅ Successfully imported {} new sessions! ({} skipped as duplicate, total {} parsed)",
                                summary.new_imported, summary.duplicates_skipped, summary.total_parsed),
                            false
                        ));
                    }

                    if ui.button(RichText::new("Clear Text").color(Color32::LIGHT_GRAY)).clicked() {
                        self.import_raw_text.clear();
                        self.import_status_msg = None;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
            });

        if close {
            self.show_import_dialog = false;
        }
    }
}

impl eframe::App for RustyBoxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Enforce UI zoom / font scale factor
        ctx.set_zoom_factor(self.ui_scale);

        // Global Keyboard Shortcuts for UI Zoom & Font Scale
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Plus)
            || i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Equals)) {
            self.ui_scale = (self.ui_scale + 0.1).min(2.0);
            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Minus)) {
            self.ui_scale = (self.ui_scale - 0.1).max(0.7);
            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Num0)
            || i.consume_key(egui::Modifiers::CTRL, egui::Key::Num0)) {
            self.ui_scale = 1.0;
            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
        }

        // Global Keyboard Shortcuts
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.search_open = true;
            self.focus_search = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::I) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::I)) {
            self.show_import_dialog = true;
        }
        let active_tab_dead = !self.is_home_active
            && self
                .open_tabs
                .get(self.active_tab_idx)
                .map(|t| !t.session.is_alive())
                .unwrap_or(false);
        let close_tab_shortcut = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::W)
                || i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::W,
                )
                || (active_tab_dead
                    && (i.consume_key(egui::Modifiers::CTRL, egui::Key::W)
                        || i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)))
        });
        if close_tab_shortcut {
            if !self.open_tabs.is_empty() && !self.is_home_active {
                let active = self.active_tab_idx;
                if active < self.open_tabs.len() {
                    let mut closed = self.open_tabs.remove(active);
                    closed.session.kill();
                    if self.open_tabs.is_empty() {
                        self.is_home_active = true;
                    } else if self.active_tab_idx >= self.open_tabs.len() {
                        self.active_tab_idx = self.open_tabs.len() - 1;
                    }
                }
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::T) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.spawn_local_terminal("Local Terminal", None, None);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_network_tools = false;
            self.show_services_modal = false;
            self.show_tunnels = false;
            self.show_tunnel_edit = false;
            self.show_keygen = false;
            self.show_pwdgen = false;
            self.show_diff = false;
            self.show_mobapt = false;
            self.show_macro_manager = false;
            self.show_vault = false;
            self.show_settings_dialog = false;
            self.show_hotkeys_dialog = false;
            self.show_about = false;
            self.show_manage_passwords = false;
            self.show_import_dialog = false;
            self.new_session_dialog.is_open = false;
            self.connect_as_target = None;
            self.rename_session_target = None;
            self.rename_folder_target = None;
            self.pending_paste_text = None;
            self.search_open = false;
        }
        self.handle_multi_exec_broadcast();
        self.update_telemetry();

        // Smart Repaint: Repaint immediately only if VISIBLE active terminal(s) have new output
        let visible_has_data = match self.split_mode {
            SplitMode::Single => {
                if !self.is_home_active && !self.open_tabs.is_empty() {
                    let idx = self.active_tab_idx.min(self.open_tabs.len() - 1);
                    self.open_tabs[idx].session.has_new_data.swap(false, std::sync::atomic::Ordering::Relaxed)
                } else {
                    false
                }
            }
            SplitMode::Horizontal | SplitMode::Vertical => {
                let mut d = false;
                if !self.is_home_active && !self.open_tabs.is_empty() {
                    let idx_a = self.active_tab_idx.min(self.open_tabs.len() - 1);
                    d |= self.open_tabs[idx_a].session.has_new_data.swap(false, std::sync::atomic::Ordering::Relaxed);
                    if self.open_tabs.len() > 1 {
                        let idx_b = if idx_a == 0 { 1 } else { 0 };
                        d |= self.open_tabs[idx_b].session.has_new_data.swap(false, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                d
            }
            SplitMode::Grid2x2 => {
                let mut d = false;
                if !self.is_home_active && !self.open_tabs.is_empty() {
                    for i in 0..4.min(self.open_tabs.len()) {
                        d |= self.open_tabs[i].session.has_new_data.swap(false, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                d
            }
        };

        // Clear has_new_data on background inactive tabs so it doesn't linger
        for (i, t) in self.open_tabs.iter().enumerate() {
            if i != self.active_tab_idx {
                t.session.has_new_data.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }

        if visible_has_data || self.multi_exec_enabled {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // 1. TOP MENU BAR (Branded Header + Menus + Quick Utilities)
        TopBottomPanel::top("top_menu")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(14, 18, 26))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(26, 32, 46)))
                    .inner_margin(egui::Margin::symmetric(10.0, 4.0))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("⚡ RustyBox")
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong()
                            .size(13.5),
                    );
                    egui::Frame::none()
                        .fill(Color32::from_rgb(22, 30, 44))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(38, 54, 80)))
                        .rounding(4.0)
                        .inner_margin(egui::Margin::symmetric(5.0, 1.5))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("v1.3-55")
                                    .color(Color32::from_rgb(147, 197, 253))
                                    .size(10.5)
                                    .strong(),
                            );
                        });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

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
                    if ui.button("📁 Import MobaXterm Sessions (.txt / .ini / .mxtpro)...").clicked() {
                        self.show_import_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("📥 Import Sessions from File (JSON)...").clicked() {
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
                    ui.menu_button("🔍 UI Font & Zoom Scale", |ui| {
                        if ui.button("➕ Zoom In UI (+10%)\tCtrl+Shift++").clicked() {
                            self.ui_scale = (self.ui_scale + 0.1).min(2.0);
                            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                            ui.close_menu();
                        }
                        if ui.button("➖ Zoom Out UI (-10%)\tCtrl+Shift+-").clicked() {
                            self.ui_scale = (self.ui_scale - 0.1).max(0.7);
                            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                            ui.close_menu();
                        }
                        if ui.button("↺ Reset Zoom (100%)\tCtrl+0").clicked() {
                            self.ui_scale = 1.0;
                            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                            ui.close_menu();
                        }
                        ui.separator();
                        for scale in [0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0] {
                            let label = format!("{:.0}%{}", scale * 100.0, if (self.ui_scale - scale).abs() < 0.02 { "  ✓" } else { "" });
                            if ui.selectable_label((self.ui_scale - scale).abs() < 0.02, label).clicked() {
                                self.ui_scale = scale;
                                let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                                ui.close_menu();
                            }
                        }
                    });
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
                    if ui.button("⚡ Wake-on-LAN (WOL)").clicked() {
                        self.show_network_tools = true;
                        self.net_tools_tab = 5;
                        ui.close_menu();
                    }
                    if ui.button("🌐 DNS Lookup").clicked() {
                        self.show_network_tools = true;
                        self.net_tools_tab = 4;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🚀 Embedded Daemons (HTTP, Syslog, TFTP)").clicked() {
                        self.show_services_modal = true;
                        ui.close_menu();
                    }
                    if ui.button("🔀 SSH Tunnels / Port Forwarding").clicked() {
                        self.show_tunnels = true;
                        ui.close_menu();
                    }
                    if ui.button("🔑 SSH Key Generator (RustyBox KeyGen)").clicked() {
                        self.show_keygen = true;
                        ui.close_menu();
                    }
                    if ui.button("🗝 Random Password Generator").clicked() {
                        self.show_pwdgen = true;
                        ui.close_menu();
                    }
                    if ui.button("📄 File Comparison (RustyBox Diff)").clicked() {
                        self.show_diff = true;
                        ui.close_menu();
                    }
                    if ui.button("⚡ Manage Sysadmin Macros...").clicked() {
                        self.show_macro_manager = true;
                        ui.close_menu();
                    }
                    if ui.button("📦 RustyBox Apt Package Manager...").clicked() {
                        self.show_mobapt = true;
                        ui.close_menu();
                    }
                    if ui.button("🔒 Master Password & Vault").clicked() {
                        self.show_vault = true;
                        ui.close_menu();
                    }
                });

                // SETTINGS MENU (MobaXterm TFORMPARAMS & TFormManagePasswords)
                ui.menu_button("Settings", |ui| {
                    if ui.button("⚙ Configuration / Preferences...").clicked() {
                        self.show_settings_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("🔑 Manage Passwords...").clicked() {
                        self.show_manage_passwords = true;
                        ui.close_menu();
                    }
                    if ui.button("🔒 Master Password Settings...").clicked() {
                        self.show_vault = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("📋 Paste Warning Settings...").clicked() {
                        self.show_settings_dialog = true;
                        self.settings_tab = 1;
                        ui.close_menu();
                    }
                    if ui.button("🖱 Windows Right-Click Menu Integration...").clicked() {
                        self.show_settings_dialog = true;
                        self.settings_tab = 6;
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

            // Quick utilities on the right side of Menu Bar
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let util_btn = |ui: &mut egui::Ui, text: &str| -> egui::Response {
                    ui.add(
                        egui::Button::new(
                            RichText::new(text)
                                .color(Color32::from_rgb(210, 220, 235))
                                .size(11.5),
                        )
                        .fill(Color32::from_rgb(20, 26, 38))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(34, 44, 62)))
                        .rounding(5.0),
                    )
                };

                if util_btn(ui, "⚙ Settings").clicked() {
                    self.show_settings_dialog = true;
                }
                if util_btn(ui, "🔑 KeyGen").clicked() {
                    self.show_keygen = true;
                }
                if util_btn(ui, "🗝 PwdGen").clicked() {
                    self.show_pwdgen = true;
                }
                if util_btn(ui, "📁 Files").clicked() {
                    self.show_sftp_sidebar = !self.show_sftp_sidebar;
                    self.sidebar_tab = 1;
                }
                if util_btn(ui, "🔀 Tunnels").clicked() {
                    self.show_tunnels = true;
                }
                if util_btn(ui, "🕊 Daemons").clicked() {
                    self.show_services_modal = true;
                }
                if util_btn(ui, "🌐 NetTools").clicked() {
                    self.show_network_tools = true;
                }
            });
        });
    });

        // 2. MAIN TOOLBAR (Unified Modern Toolbar Suite)
        TopBottomPanel::top("main_toolbar")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(16, 20, 29))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(26, 33, 47)))
                    .inner_margin(egui::Margin::symmetric(10.0, 5.0))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let toolbar_h = 26.0;

                    let tb_button = |ui: &mut egui::Ui, text: &str, active: bool| -> egui::Response {
                        let fill = if active {
                            Color32::from_rgb(37, 99, 235)
                        } else {
                            Color32::from_rgb(22, 28, 40)
                        };
                        let stroke = if active {
                            egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250))
                        } else {
                            egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64))
                        };
                        let text_color = if active {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(205, 215, 228)
                        };
                        let btn = egui::Button::new(RichText::new(text).color(text_color).size(12.0))
                            .fill(fill)
                            .stroke(stroke)
                            .rounding(6.0)
                            .min_size(egui::vec2(0.0, toolbar_h));
                        ui.add(btn)
                    };

                    // Prominent Modern New Session Button
                    let new_sess_btn = egui::Button::new(
                        RichText::new("➕ New Session").color(Color32::WHITE).size(12.0).strong()
                    )
                    .fill(Color32::from_rgb(37, 99, 235))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)))
                    .rounding(6.0)
                    .min_size(egui::vec2(0.0, toolbar_h));
                    if ui.add(new_sess_btn).clicked() {
                        self.new_session_dialog.open_for_new();
                    }

                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // Modern Linear-style Segmented Layout Control
                    egui::Frame::none()
                        .fill(Color32::from_rgb(12, 15, 22))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 38, 54)))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(2.0, 2.0))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let seg_btn = |ui: &mut egui::Ui, text: &str, active: bool| -> egui::Response {
                                let (fill, stroke, text_color) = if active {
                                    (
                                        Color32::from_rgb(37, 99, 235),
                                        egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)),
                                        Color32::WHITE,
                                    )
                                } else {
                                    (
                                        Color32::TRANSPARENT,
                                        egui::Stroke::NONE,
                                        Color32::from_rgb(160, 175, 195),
                                    )
                                };
                                ui.add(
                                    egui::Button::new(RichText::new(text).color(text_color).size(11.5))
                                        .fill(fill)
                                        .stroke(stroke)
                                        .rounding(4.0)
                                        .min_size(egui::vec2(0.0, toolbar_h - 4.0))
                                )
                            };
                            if seg_btn(ui, "🗖 Single", self.split_mode == SplitMode::Single).clicked() {
                                self.split_mode = SplitMode::Single;
                            }
                            if seg_btn(ui, "◫ 2-Cols", self.split_mode == SplitMode::Horizontal).clicked() {
                                self.split_mode = SplitMode::Horizontal;
                            }
                            if seg_btn(ui, "⬒ 2-Rows", self.split_mode == SplitMode::Vertical).clicked() {
                                self.split_mode = SplitMode::Vertical;
                            }
                            if seg_btn(ui, "⊞ 2x2 Grid", self.split_mode == SplitMode::Grid2x2).clicked() {
                                self.split_mode = SplitMode::Grid2x2;
                            }
                        });

                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // Multi-Exec toggle
                    let multi_btn = if self.multi_exec_enabled {
                        egui::Button::new(RichText::new("⚡ Multi-Exec: ON").color(Color32::WHITE).size(12.0).strong())
                            .fill(Color32::from_rgb(217, 119, 6))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(251, 191, 36)))
                    } else {
                        egui::Button::new(RichText::new("⚡ Multi-Exec: OFF").color(Color32::from_rgb(148, 163, 184)).size(12.0))
                            .fill(Color32::from_rgb(22, 28, 40))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                    }
                    .rounding(6.0)
                    .min_size(egui::vec2(0.0, toolbar_h));
                    if ui.add(multi_btn).on_hover_text("Broadcast input to all open split terminals simultaneously").clicked() {
                        self.multi_exec_enabled = !self.multi_exec_enabled;
                    }

                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // Find in terminal
                    if tb_button(ui, "🔍 Find", self.search_open).clicked() {
                        self.search_open = !self.search_open;
                    }

                    // Dual UI Scale & Terminal Font stepper in modern unified container
                    egui::Frame::none()
                        .fill(Color32::from_rgb(22, 28, 40))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                        .show(ui, |ui| {
                            ui.set_height(toolbar_h);
                            ui.horizontal(|ui| {
                                // UI Font / Scale Zoom
                                ui.label(RichText::new("UI:").color(Color32::from_rgb(148, 163, 184)).size(11.0));
                                if ui.small_button("➖").on_hover_text("Zoom out UI font (Ctrl+Shift+-)").clicked() && self.ui_scale > 0.75 {
                                    self.ui_scale = (self.ui_scale - 0.1).max(0.7);
                                    let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                                }
                                let pct = (self.ui_scale * 100.0).round() as i32;
                                if ui.button(RichText::new(format!("{}%", pct)).size(11.0).strong())
                                    .on_hover_text("Click to reset UI Zoom to 100%")
                                    .clicked() {
                                    self.ui_scale = 1.0;
                                    let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                                }
                                if ui.small_button("➕").on_hover_text("Zoom in UI font (Ctrl+Shift++)").clicked() && self.ui_scale < 2.0 {
                                    self.ui_scale = (self.ui_scale + 0.1).min(2.0);
                                    let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                                }

                                ui.separator();

                                // Terminal Font Size
                                ui.label(RichText::new("Term:").color(Color32::from_rgb(148, 163, 184)).size(11.0));
                                if ui.small_button("➖").on_hover_text("Decrease terminal font size (Ctrl+-)").clicked() && self.font_size > 8.0 {
                                    self.font_size -= 1.0;
                                    let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                                }
                                ui.label(RichText::new(format!("{:.0}px", self.font_size)).size(11.0).strong());
                                if ui.small_button("➕").on_hover_text("Increase terminal font size (Ctrl++)").clicked() && self.font_size < 32.0 {
                                    self.font_size += 1.0;
                                    let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
                                }
                            });
                        });

                    // Theme selector in a modern unified container
                    egui::Frame::none()
                        .fill(Color32::from_rgb(22, 28, 40))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                        .show(ui, |ui| {
                            ui.set_height(toolbar_h);
                            egui::ComboBox::from_id_salt("toolbar_theme_select")
                                .selected_text(RichText::new(self.theme.name()).size(11.0))
                                .show_ui(ui, |ui| {
                                    for t in TerminalTheme::all() {
                                        ui.selectable_value(&mut self.theme, *t, t.name());
                                    }
                                });
                        });

                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // Master Password Vault status indicator
                    let is_vault_configured = self.storage.is_vault_configured();
                    let is_unlocked = self.storage.is_vault_unlocked();
                    let (v_icon, v_txt, v_col) = if !is_vault_configured {
                        ("🔑", "Master Key: Off", Color32::from_rgb(148, 163, 184))
                    } else if is_unlocked {
                        ("🔓", "Vault: Unlocked", Color32::from_rgb(34, 197, 94))
                    } else {
                        ("🔒", "Vault: Locked", Color32::from_rgb(251, 191, 36))
                    };
                    if ui.add(
                        egui::Button::new(RichText::new(format!("{} {}", v_icon, v_txt)).size(11.0).color(v_col).strong())
                            .fill(Color32::from_rgb(22, 28, 40))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                            .rounding(6.0)
                            .min_size(egui::vec2(0.0, toolbar_h))
                    ).on_hover_text("Manage Master Password & AES-256-GCM Vault").clicked() {
                        self.show_vault = true;
                    }
                });
            });

        // 3. COMPACT BOTTOM PANEL (Quick Macros + Real-time System Telemetry)
        TopBottomPanel::bottom("bottom_bar")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(14, 18, 25))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(26, 33, 46)))
                    .inner_margin(egui::Margin::symmetric(10.0, 3.5))
            )
            .min_height(26.0)
            .show(ctx, |ui| {
                let avail_w = ui.available_width();
                let telem_w = 660.0f32.min(avail_w * 0.55);
                let macros_w = (avail_w - telem_w - 12.0).max(100.0);

                let bottom_rect = ui.available_rect_before_wrap();
                let macros_rect = egui::Rect::from_min_max(bottom_rect.min, pos2(bottom_rect.min.x + macros_w, bottom_rect.max.y));
                let telem_rect = egui::Rect::from_min_max(pos2(bottom_rect.max.x - telem_w, bottom_rect.min.y), bottom_rect.max);

                // LEFT: QUICK MACROS (Scrollable without scrollbar so it never collides with telemetry)
                let mut macros_ui = ui.new_child(egui::UiBuilder::new().max_rect(macros_rect).layout(Layout::left_to_right(Align::Center)));
                egui::ScrollArea::horizontal()
                    .id_salt("quick_macros_scroll")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(&mut macros_ui, |ui| {
                        ui.label(RichText::new("⚡ Quick:").weak().size(11.0).color(Color32::from_rgb(148, 163, 184)));
                        let mut macro_to_run: Option<String> = None;
                        for (name, cmd) in &self.quick_macros {
                            let pill = egui::Button::new(
                                RichText::new(name).size(11.0).color(Color32::from_rgb(220, 230, 245))
                            )
                            .fill(Color32::from_rgb(22, 28, 40))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                            .rounding(6.0);
                            if ui.add(pill).clicked() {
                                macro_to_run = Some(cmd.clone());
                            }
                        }
                        if ui.add(
                            egui::Button::new(RichText::new("⚙ Manage").size(11.0).color(Color32::from_rgb(148, 163, 184)))
                                .fill(Color32::from_rgb(18, 23, 33))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(32, 40, 56)))
                                .rounding(5.0)
                        ).clicked() {
                            self.show_macro_manager = true;
                        }
                        if let Some(cmd) = macro_to_run {
                            self.execute_macro(&cmd);
                        }
                    });

                // RIGHT: LIVE SYSTEM TELEMETRY MONITOR (per active tab / server)
                let mut telem_ui = ui.new_child(egui::UiBuilder::new().max_rect(telem_rect).layout(Layout::right_to_left(Align::Center)));
                {
                    let ui = &mut telem_ui;
                    let telem = self.get_active_telemetry();
                    let stat_chip = |ui: &mut egui::Ui, text: RichText| {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(18, 24, 35))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 40, 58)))
                            .rounding(5.0)
                            .inner_margin(egui::Margin::symmetric(6.0, 1.5))
                            .show(ui, |ui| {
                                ui.label(text);
                            });
                    };

                        // 1. Host (Far right)
                        stat_chip(ui, RichText::new(format!("🖥 {}", telem.host)).strong().size(10.5).color(Color32::WHITE));

                        // 2. CPU with visual gauge
                        let cpu_frac = (telem.cpu_pct as f32 / 100.0).clamp(0.0, 1.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgb(18, 24, 35))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 40, 58)))
                            .rounding(5.0)
                            .inner_margin(egui::Margin::symmetric(7.0, 2.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("⚡ {}%", telem.cpu_pct)).size(10.5).color(Color32::from_rgb(220, 230, 245)));
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 6.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, 2.5, Color32::from_rgb(30, 38, 52));
                                    let mut filled_rect = rect;
                                    filled_rect.set_width(rect.width() * cpu_frac);
                                    let bar_color = if telem.cpu_pct > 75 {
                                        Color32::from_rgb(239, 68, 68)
                                    } else if telem.cpu_pct > 50 {
                                        Color32::from_rgb(245, 158, 11)
                                    } else {
                                        Color32::from_rgb(34, 197, 94)
                                    };
                                    ui.painter().rect_filled(filled_rect, 2.5, bar_color);
                                });
                            });

                        // 3. Cores
                        stat_chip(ui, RichText::new(format!("⚙ {} Cores", telem.cores)).color(Color32::from_rgb(167, 139, 250)).size(10.5));

                        // 4. RAM with visual gauge
                        let ram_pct = (telem.ram_used_gb / telem.ram_total_gb.max(0.1)).clamp(0.0, 1.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgb(18, 24, 35))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(30, 40, 58)))
                            .rounding(5.0)
                            .inner_margin(egui::Margin::symmetric(7.0, 2.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("💾 {:.1}/{:.1}G", telem.ram_used_gb, telem.ram_total_gb)).size(10.5).color(Color32::from_rgb(220, 230, 245)));
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 6.0), egui::Sense::hover());
                                    ui.painter().rect_filled(rect, 2.5, Color32::from_rgb(30, 38, 52));
                                    let mut filled_rect = rect;
                                    filled_rect.set_width(rect.width() * ram_pct);
                                    ui.painter().rect_filled(filled_rect, 2.5, Color32::from_rgb(56, 189, 248));
                                });
                            });

                        // 5. Network throughput
                        stat_chip(ui, RichText::new(format!("🌐 {}", telem.net_rate)).color(Color32::from_rgb(56, 189, 248)).size(10.5));

                        // 6. Uptime
                        stat_chip(ui, RichText::new(format!("⏱ {}", telem.uptime)).size(10.5).color(Color32::from_rgb(156, 163, 175)));

                        // 7. User
                        stat_chip(ui, RichText::new(format!("👤 {}", telem.user)).size(10.5).color(Color32::from_rgb(201, 209, 217)));

                        // 8. Disk usage
                        stat_chip(
                            ui,
                            RichText::new(format!("🖴 {}%", telem.disk_pct))
                                .color(Color32::from_rgb(74, 222, 128))
                                .size(10.5),
                        );

                        // 9. Multi-Exec Warning
                        if self.multi_exec_enabled {
                            stat_chip(ui, RichText::new("⚠ MULTI-EXEC").color(Color32::from_rgb(251, 191, 36)).size(10.5).strong());
                        }

                        // 10. Connection status
                        if self.open_tabs.is_empty() || self.is_home_active {
                            stat_chip(ui, RichText::new("○ Ready").weak().size(10.5));
                        } else if telem.is_connected {
                            stat_chip(ui, RichText::new("● Connected").color(Color32::from_rgb(34, 197, 94)).size(10.5).strong());
                        } else {
                            stat_chip(ui, RichText::new("✕ Disconnected").color(Color32::from_rgb(239, 68, 68)).size(10.5).strong());
                        }
                    }
            });

        // 5. LEFT SIDEBAR (Sessions Tree & SFTP Explorer)
        if self.show_sftp_sidebar {
            egui::SidePanel::left("left_sidebar")
                .frame(
                    egui::Frame::none()
                        .fill(Color32::from_rgb(14, 18, 25))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(26, 33, 46)))
                        .inner_margin(egui::Margin::symmetric(8.0, 8.0))
                )
                .resizable(true)
                .default_width(260.0)
                .width_range(180.0..=450.0)
                .show(ctx, |ui| {
                    // Modern segmented pill switcher
                    egui::Frame::none()
                        .fill(Color32::from_rgb(10, 13, 19))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(28, 36, 50)))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(2.0, 2.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                let btn_w = (ui.available_width() - 2.0) / 2.0;
                                let s_active = self.sidebar_tab == 0;
                                let sftp_active = self.sidebar_tab == 1;

                                let (s_fill, s_stroke, s_text) = if s_active {
                                    (Color32::from_rgb(37, 99, 235), egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)), Color32::WHITE)
                                } else {
                                    (Color32::TRANSPARENT, egui::Stroke::NONE, Color32::from_rgb(148, 163, 184))
                                };
                                let (sftp_fill, sftp_stroke, sftp_text) = if sftp_active {
                                    (Color32::from_rgb(37, 99, 235), egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)), Color32::WHITE)
                                } else {
                                    (Color32::TRANSPARENT, egui::Stroke::NONE, Color32::from_rgb(148, 163, 184))
                                };

                                if ui.add_sized([btn_w, 24.0], egui::Button::new(RichText::new("📂 Sessions").size(11.0).strong().color(s_text)).fill(s_fill).stroke(s_stroke).rounding(5.0)).clicked() {
                                    self.sidebar_tab = 0;
                                }
                                if ui.add_sized([btn_w, 24.0], egui::Button::new(RichText::new("📁 SFTP Browser").size(11.0).strong().color(sftp_text)).fill(sftp_fill).stroke(sftp_stroke).rounding(5.0)).clicked() {
                                    self.sidebar_tab = 1;
                                }
                            });
                        });
                    ui.add_space(5.0);
                    ui.separator();
                    ui.add_space(5.0);

                    if self.sidebar_tab == 0 {
                        // SESSIONS TREE
                        ui.horizontal(|ui| {
                            let filter_hint = format!("Filter {} sessions...", self.sessions.len());
                            ui.add(
                                TextEdit::singleline(&mut self.session_search)
                                    .hint_text(filter_hint)
                                    .desired_width(ui.available_width() - 62.0)
                            );
                            let add_btn = egui::Button::new(RichText::new("➕").color(Color32::WHITE).size(12.0))
                                .fill(Color32::from_rgb(37, 99, 235))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)))
                                .rounding(5.0);
                            if ui.add(add_btn).on_hover_text("New Session").clicked() {
                                self.new_session_dialog.open_for_new();
                            }
                            let imp_btn = egui::Button::new(RichText::new("📥").color(Color32::WHITE).size(12.0))
                                .fill(Color32::from_rgb(22, 28, 40))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                                .rounding(5.0);
                            if ui.add(imp_btn).on_hover_text("Import MobaXterm Sessions...").clicked() {
                                self.show_import_dialog = true;
                            }
                        });
                        ui.add_space(6.0);

                        ui.label(RichText::new("★ User sessions").color(Color32::from_rgb(234, 179, 8)).strong().size(12.0));
                        ui.add_space(2.0);

                        let mut grouped: BTreeMap<String, Vec<Session>> = BTreeMap::new();
                        for s in &self.sessions {
                            if self.session_search.is_empty()
                                || s.name.to_lowercase().contains(&self.session_search.to_lowercase())
                            {
                                grouped.entry(s.folder.clone()).or_default().push(s.clone());
                            }
                        }

                        let mut to_launch: Option<Session> = None;
                        let mut to_connect_as: Option<(Session, String)> = None;
                        let mut to_ping_host: Option<String> = None;
                        let mut to_rename: Option<(String, String)> = None;
                        let mut to_edit: Option<Session> = None;
                        let mut to_delete_id: Option<String> = None;
                        let mut to_duplicate: Option<Session> = None;
                        let mut to_save_file: Option<Session> = None;
                        let mut to_create_shortcut: Option<Session> = None;
                        let mut to_save_preset: Option<Session> = None;
                        let mut to_copy_settings: Option<Session> = None;
                        let mut to_new_in_folder: Option<String> = None;
                        let mut to_rename_folder: Option<(String, String)> = None;
                        let mut to_delete_folder: Option<String> = None;

                        ScrollArea::vertical().show(ui, |ui| {
                            for (folder, list) in grouped {
                                let folder_name = folder.clone();
                                let header = egui::CollapsingHeader::new(
                                    RichText::new(format!("📁 {} ({})", folder, list.len())).strong().color(Color32::from_rgb(201, 209, 217))
                                )
                                .default_open(true);

                                let res = header.show(ui, |ui| {
                                    for s in list {
                                        let icon = match s.session_type {
                                            SessionType::LocalShell { .. } => ">_ ",
                                            SessionType::Ssh { .. } => "🔑 ",
                                            SessionType::Serial { .. } => "🔌 ",
                                            SessionType::Telnet { .. } => "📡 ",
                                            SessionType::Rdp { .. } => "🖥 ",
                                            SessionType::Vnc { .. } => "👁 ",
                                            SessionType::Ftp { .. } => "📁 ",
                                            SessionType::Wsl { .. } => "🐧 ",
                                        };

                                        let subtitle = match &s.session_type {
                                            SessionType::LocalShell { .. } => "local terminal".to_string(),
                                            SessionType::Ssh { host, user, port, .. } => format!("{}@{}:{}", user, host, port),
                                            SessionType::Telnet { host, port } => format!("telnet://{}:{}", host, port),
                                            SessionType::Rdp { host, port, .. } => format!("rdp://{}:{}", host, port),
                                            SessionType::Vnc { host, port, .. } => format!("vnc://{}:{}", host, port),
                                            SessionType::Ftp { host, port, user } => format!("ftp://{}@{}:{}", user, host, port),
                                            SessionType::Serial { port_name, baud_rate, .. } => format!("{} @ {} baud", port_name, baud_rate),
                                            SessionType::Wsl { distro, .. } => format!("wsl ({})", distro),
                                        };

                                        let is_connected_tab = self.open_tabs.iter().any(|t| {
                                            t.session_def.as_ref().map(|def| def.id == s.id).unwrap_or(false) && t.session.is_alive()
                                        });
                                        let is_selected = self.selected_sidebar_session_id.as_deref() == Some(&s.id)
                                            || (self.selected_sidebar_session_id.is_none()
                                                && !self.is_home_active
                                                && self.open_tabs.get(self.active_tab_idx).and_then(|t| t.session_def.as_ref()).map(|def| def.id == s.id).unwrap_or(false));

                                        let id = ui.make_persistent_id(&s.id);
                                        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), egui::Sense::hover());
                                        let resp = ui.interact(rect, id, egui::Sense::click());

                                        if resp.clicked() || resp.secondary_clicked() {
                                            self.selected_sidebar_session_id = Some(s.id.clone());
                                        }

                                        let (bg, stroke) = if is_selected {
                                            (Color32::from_rgb(37, 99, 235), egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)))
                                        } else if resp.hovered() {
                                            (Color32::from_rgb(24, 32, 46), egui::Stroke::new(1.0, Color32::from_rgb(38, 50, 72)))
                                        } else {
                                            (Color32::TRANSPARENT, egui::Stroke::NONE)
                                        };
                                        ui.painter().rect(rect, 6.0, bg, stroke);

                                        let text_color = if is_selected { Color32::WHITE } else { Color32::from_rgb(220, 230, 242) };
                                        let sub_color = if is_selected { Color32::from_rgb(200, 225, 255) } else { Color32::from_rgb(139, 148, 158) };
                                        let icon_color = if is_selected { Color32::WHITE } else { Color32::from_rgb(56, 189, 248) };

                                        ui.painter().text(
                                            egui::pos2(rect.left() + 8.0, rect.top() + 10.0),
                                            egui::Align2::LEFT_CENTER,
                                            icon,
                                            egui::FontId::proportional(12.0),
                                            icon_color,
                                        );
                                        ui.painter().text(
                                            egui::pos2(rect.left() + 28.0, rect.top() + 10.0),
                                            egui::Align2::LEFT_CENTER,
                                            &s.name,
                                            egui::FontId::proportional(12.0),
                                            text_color,
                                        );
                                        ui.painter().text(
                                            egui::pos2(rect.left() + 28.0, rect.top() + 24.0),
                                            egui::Align2::LEFT_CENTER,
                                            &subtitle,
                                            egui::FontId::proportional(10.5),
                                            sub_color,
                                        );
                                        if is_connected_tab {
                                            let dot_pos = egui::pos2(rect.right() - 14.0, rect.center().y);
                                            ui.painter().circle_filled(dot_pos, 4.0, Color32::from_rgb(34, 197, 94));
                                        }

                                        if resp.double_clicked() || (self.selected_sidebar_session_id.as_deref() == Some(&s.id) && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                            to_launch = Some(s.clone());
                                        }
                                        resp.context_menu(|ui| {
                                            ui.set_min_width(210.0);
                                            if ui.button(RichText::new("★ Execute").strong().color(Color32::from_rgb(234, 179, 8))).clicked() {
                                                to_launch = Some(s.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button("👤 Connect as...").clicked() {
                                                let cur_user = match &s.session_type {
                                                    SessionType::Ssh { user, .. } => user.clone(),
                                                    _ => "root".to_string(),
                                                };
                                                to_connect_as = Some((s.clone(), cur_user));
                                                ui.close_menu();
                                            }
                                            if let Some(h) = s.host_str() {
                                                if ui.button("🌐 Ping host").clicked() {
                                                    to_ping_host = Some(h);
                                                    ui.close_menu();
                                                }
                                            }
                                            ui.separator();
                                            if ui.button("📝 Rename session").clicked() {
                                                to_rename = Some((s.id.clone(), s.name.clone()));
                                                ui.close_menu();
                                            }
                                            if ui.button("⚙ Edit session").clicked() {
                                                to_edit = Some(s.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button(RichText::new("× Delete session").strong().color(Color32::from_rgb(239, 68, 68))).clicked() {
                                                to_delete_id = Some(s.id.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button("🔑 Duplicate session").clicked() {
                                                to_duplicate = Some(s.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button("💾 Save session to file").clicked() {
                                                to_save_file = Some(s.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button("☆ Create a desktop shortcut").clicked() {
                                                to_create_shortcut = Some(s.clone());
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                            if ui.button("📥 Save session settings as default presets").clicked() {
                                                to_save_preset = Some(s.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button("📋 Copy session settings").clicked() {
                                                to_copy_settings = Some(s.clone());
                                                ui.close_menu();
                                            }
                                        });
                                    }
                                });

                                res.header_response.context_menu(|ui| {
                                    if ui.button("➕ New session in this folder...").clicked() {
                                        to_new_in_folder = Some(folder_name.clone());
                                        ui.close_menu();
                                    }
                                    if ui.button("📁 Rename folder...").clicked() {
                                        to_rename_folder = Some((folder_name.clone(), folder_name.clone()));
                                        ui.close_menu();
                                    }
                                    if ui.button(RichText::new("✕ Delete folder").color(Color32::from_rgb(239, 68, 68))).clicked() {
                                        to_delete_folder = Some(folder_name.clone());
                                        ui.close_menu();
                                    }
                                });
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.label(RichText::new("Double-click to connect • Right-click for menu").weak().size(10.0));
                        });

                        if let Some(s) = to_launch {
                            self.launch_generic_session(s);
                        }
                        if let Some((s, u)) = to_connect_as {
                            self.connect_as_target = Some((s, u));
                        }
                        if let Some(h) = to_ping_host {
                            self.show_network_tools = true;
                            self.net_tools_tab = 2;
                            self.network_tools.ping_host = h;
                            self.network_tools.ping_logs.lock().clear();
                        }
                        if let Some((id, name)) = to_rename {
                            self.rename_session_target = Some((id, name));
                        }
                        if let Some(s) = to_edit {
                            self.new_session_dialog.open_for_edit(&s);
                        }
                        if let Some(del_id) = to_delete_id {
                            self.sessions.retain(|item| item.id != del_id);
                            let _ = self.storage.save_sessions(&self.sessions);
                        }
                        if let Some(dup_s) = to_duplicate {
                            let mut copy = dup_s.clone();
                            copy.id = uuid::Uuid::new_v4().to_string();
                            copy.name = format!("{} (Copy)", copy.name);
                            self.sessions.push(copy);
                            let _ = self.storage.save_sessions(&self.sessions);
                        }
                        if let Some(save_s) = to_save_file {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&format!("{}.json", save_s.name.replace('/', "_")))
                                .add_filter("JSON session", &["json"])
                                .save_file()
                            {
                                if let Ok(json) = serde_json::to_string_pretty(&save_s) {
                                    let _ = std::fs::write(path, json);
                                }
                            }
                        }
                        if let Some(sc_s) = to_create_shortcut {
                            #[cfg(target_os = "windows")]
                            {
                                if let Some(desktop) = dirs::desktop_dir() {
                                    let safe_name = sc_s.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
                                    let bat_path = desktop.join(format!("{}.bat", safe_name));
                                    let content = format!("@echo off\r\nstart \"\" \"%~dp0RustyBox.exe\" --session \"{}\"\r\n", sc_s.name);
                                    let _ = std::fs::write(&bat_path, content);
                                    self.status_toast = Some((format!("Shortcut created: {}.bat", safe_name), std::time::Instant::now()));
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                if let Some(desktop) = dirs::desktop_dir() {
                                    let safe_name = sc_s.name.replace('/', "_");
                                    let sh_path = desktop.join(format!("{}.sh", safe_name));
                                    let content = format!("#!/bin/sh\nrustybox --session \"{}\"\n", sc_s.name);
                                    let _ = std::fs::write(&sh_path, content);
                                    self.status_toast = Some((format!("Shortcut created: {}.sh", safe_name), std::time::Instant::now()));
                                }
                            }
                        }
                        if let Some(preset_s) = to_save_preset {
                            self.status_toast = Some((format!("Saved preset for '{}'", preset_s.name), std::time::Instant::now()));
                        }
                        if let Some(copy_s) = to_copy_settings {
                            if let Ok(json) = serde_json::to_string(&copy_s) {
                                ctx.output_mut(|o| o.copied_text = json);
                                self.status_toast = Some(("Session settings copied to clipboard".into(), std::time::Instant::now()));
                            }
                        }
                        if let Some(f) = to_new_in_folder {
                            self.new_session_dialog.open_for_new_in_folder(&f);
                        }
                        if let Some((old_f, new_f)) = to_rename_folder {
                            self.rename_folder_target = Some((old_f, new_f));
                        }
                        if let Some(f_del) = to_delete_folder {
                            self.sessions.retain(|s| s.folder != f_del);
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
            // TAB BAR HEADER (Explicitly partitioned into dedicated Tabs strip and Quick Connect area)
            let strip_height = 28.0;
            let (strip_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), strip_height), egui::Sense::hover());
            let quick_w = 265.0f32;
            let tabs_rect = egui::Rect::from_min_max(strip_rect.min, pos2(strip_rect.max.x - quick_w - 6.0, strip_rect.max.y));
            let quick_rect = egui::Rect::from_min_max(pos2(strip_rect.max.x - quick_w, strip_rect.min.y), strip_rect.max);

            let mut close_tab_idx: Option<usize> = None;
            let mut switch_to_tab: Option<usize> = None;
            enum TabMenuAction { Duplicate, Restart, Rename, Close, CloseOthers, CloseRight }
            let mut tab_menu_action: Option<(usize, TabMenuAction)> = None;

            // Dynamic tab width: smoothly scales so all tabs fit cleanly without clipping
            let avail_tabs_w = (tabs_rect.width() - 85.0).max(60.0);
            let num_tabs = self.open_tabs.len().max(1);
            let dynamic_tab_w = ((avail_tabs_w - 20.0) / num_tabs as f32).clamp(100.0, 180.0);

            // 1. TABS AREA (Home + Scrollable tab strip)
            let mut tabs_ui = ui.new_child(egui::UiBuilder::new().max_rect(tabs_rect).layout(Layout::left_to_right(Align::Center)));
            ScrollArea::horizontal()
                .id_salt("tabs_scroll_area")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(&mut tabs_ui, |ui| {
                    ui.horizontal(|ui| {
                        // Home Tab (Engineered inside the exact same row as all other tabs)
                        let is_home = self.is_home_active;
                        let (home_rect, home_resp) = ui.allocate_exact_size(vec2(76.0, 26.0), egui::Sense::click());
                        let home_hovered = home_resp.hovered();

                        let (h_fill, h_stroke, h_text) = if is_home {
                            (
                                Color32::from_rgb(26, 36, 56),
                                egui::Stroke::new(1.0, Color32::from_rgb(56, 189, 248)),
                                Color32::WHITE,
                            )
                        } else if home_hovered {
                            (
                                Color32::from_rgb(28, 36, 48),
                                egui::Stroke::new(1.0, Color32::from_rgb(48, 58, 76)),
                                Color32::from_rgb(215, 225, 238),
                            )
                        } else {
                            (
                                Color32::from_rgb(20, 26, 36),
                                egui::Stroke::new(1.0, Color32::from_rgb(34, 44, 60)),
                                Color32::from_rgb(160, 172, 188),
                            )
                        };

                        // Background tab frame
                        ui.painter().rect(
                            home_rect,
                            egui::Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 },
                            h_fill,
                            h_stroke,
                        );

                        // Electric cyan top line for active Home tab
                        if is_home {
                            ui.painter().line_segment(
                                [pos2(home_rect.left() + 2.0, home_rect.top() + 1.0), pos2(home_rect.right() - 2.0, home_rect.top() + 1.0)],
                                egui::Stroke::new(2.5, Color32::from_rgb(56, 189, 248)),
                            );
                        }

                        // Centered icon and title
                        ui.painter().text(
                            home_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "🏠 Home",
                            egui::FontId::proportional(11.5),
                            h_text,
                        );

                        if home_resp.clicked() {
                            self.is_home_active = true;
                            self.selected_sidebar_session_id = None;
                        }

                        ui.add_space(2.0);

                        for (idx, tab) in self.open_tabs.iter().enumerate() {
                                let is_active = !self.is_home_active && idx == self.active_tab_idx;
                                let _tab_id = &tab.id;

                                let tab_w = dynamic_tab_w;
                                let title_w = (tab_w - 22.0).max(40.0);
                                let (title_rect, title_resp) = ui.allocate_exact_size(
                                    vec2(title_w, 26.0),
                                    egui::Sense::click(),
                                );
                                let (close_rect, close_resp) = ui.allocate_exact_size(
                                    vec2(22.0, 26.0),
                                    egui::Sense::click(),
                                );
                                let tab_rect = egui::Rect::from_min_max(title_rect.min, close_rect.max);
                                let tab_hovered = title_resp.hovered() || close_resp.hovered();

                                let (fill, stroke, text_color) = if is_active {
                                    (
                                        Color32::from_rgb(26, 36, 56),
                                        egui::Stroke::new(1.0, Color32::from_rgb(56, 189, 248)),
                                        Color32::WHITE,
                                    )
                                } else if tab_hovered {
                                    (
                                        Color32::from_rgb(28, 36, 48),
                                        egui::Stroke::new(1.0, Color32::from_rgb(48, 58, 76)),
                                        Color32::from_rgb(215, 225, 238),
                                    )
                                } else {
                                    (
                                        Color32::from_rgb(20, 26, 36),
                                        egui::Stroke::new(1.0, Color32::from_rgb(34, 44, 60)),
                                        Color32::from_rgb(160, 172, 188),
                                    )
                                };

                                // Render tab background frame with modern rounded top corners
                                ui.painter().rect(
                                    tab_rect,
                                    egui::Rounding { nw: 6.0, ne: 6.0, sw: 0.0, se: 0.0 },
                                    fill,
                                    stroke,
                                );

                                // Electric cyan top line for active tab
                                if is_active {
                                    ui.painter().line_segment(
                                        [pos2(tab_rect.left() + 2.0, tab_rect.top() + 1.0), pos2(tab_rect.right() - 2.0, tab_rect.top() + 1.0)],
                                        egui::Stroke::new(2.5, Color32::from_rgb(56, 189, 248)),
                                    );
                                }

                                // Status dot
                                let dot_center = pos2(tab_rect.left() + 9.0, tab_rect.center().y);
                                let is_alive = tab.session.is_alive();
                                let dot_col = if is_alive {
                                    Color32::from_rgb(34, 197, 94) // Green alive
                                } else {
                                    Color32::from_rgb(239, 68, 68) // Red stopped / error
                                };
                                ui.painter().circle_filled(dot_center, 3.5, dot_col);

                                // Icon
                                let icon = if tab.title.contains("SSH") {
                                    "🔒 "
                                } else if tab.title.contains("WSL") {
                                    "🐧 "
                                } else if tab.title.contains("Telnet") {
                                    "⚡ "
                                } else {
                                    ">_ "
                                };

                                // Title text with dynamic truncation based on actual tab width
                                let full_title = format!("{}{}", icon, &tab.title);
                                let clean_title = tab.title.trim_start_matches("SSH: ").trim();
                                let max_chars = (((tab_w - 38.0) / 7.0) as usize).clamp(7, 22);
                                let display_title = if clean_title.chars().count() > max_chars {
                                    let s: String = clean_title.chars().take(max_chars.saturating_sub(1)).collect();
                                    format!("{}{}\u{2026}", icon, s)
                                } else {
                                    format!("{}{}", icon, clean_title)
                                };

                                let text_pos = pos2(tab_rect.left() + 16.0, tab_rect.center().y - 6.5);
                                ui.painter().text(
                                    text_pos,
                                    Align2::LEFT_TOP,
                                    display_title,
                                    egui::FontId::proportional(11.0),
                                    text_color,
                                );

                                // Close icon rendering (crisp anti-aliased cross, no font tofu)
                                let close_center = pos2(close_rect.center().x, close_rect.center().y);
                                let r_x = 3.5;
                                if close_resp.hovered() {
                                    ui.painter().circle_filled(
                                        close_center,
                                        7.5,
                                        Color32::from_rgb(220, 38, 38),
                                    );
                                    let stroke = egui::Stroke::new(1.4, Color32::WHITE);
                                    ui.painter().line_segment([pos2(close_center.x - r_x, close_center.y - r_x), pos2(close_center.x + r_x, close_center.y + r_x)], stroke);
                                    ui.painter().line_segment([pos2(close_center.x - r_x, close_center.y + r_x), pos2(close_center.x + r_x, close_center.y - r_x)], stroke);
                                } else {
                                    let stroke = egui::Stroke::new(1.3, if is_active { Color32::from_rgb(248, 113, 113) } else { Color32::from_rgb(140, 150, 160) });
                                    ui.painter().line_segment([pos2(close_center.x - r_x, close_center.y - r_x), pos2(close_center.x + r_x, close_center.y + r_x)], stroke);
                                    ui.painter().line_segment([pos2(close_center.x - r_x, close_center.y + r_x), pos2(close_center.x + r_x, close_center.y - r_x)], stroke);
                                }

                                let tab_middle_clicked = title_resp.middle_clicked() || close_resp.middle_clicked();
                                if close_resp.clicked() || tab_middle_clicked {
                                    close_tab_idx = Some(idx);
                                } else if title_resp.clicked() {
                                    switch_to_tab = Some(idx);
                                }

                                let right_clicked = title_resp.secondary_clicked() || (title_resp.hovered() && ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary)));
                                if right_clicked {
                                    let p = ui.input(|i| i.pointer.interact_pos()).unwrap_or_else(|| pos2(title_rect.left(), title_rect.bottom() + 2.0));
                                    self.tab_context_menu = Some((idx, p));
                                }

                                let tip = format!("{}\nStatus: {}\nClick to switch • Middle-click or ✕ to close • Right-click for menu", full_title, if is_alive { "Connected" } else { "Disconnected / Stopped" });
                                title_resp.on_hover_text(tip);
                            }

                            // '+' Button right after tabs in scroll area
                            let add_btn = egui::Button::new(
                                RichText::new("+").size(15.0).strong().color(Color32::from_rgb(180, 190, 200)),
                            )
                            .fill(Color32::from_rgb(30, 36, 46))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
                            .rounding(4.0)
                            .min_size(egui::vec2(24.0, 24.0));
                            if ui.add(add_btn).on_hover_text("Open new local terminal").clicked() {
                                self.spawn_local_terminal("Local Terminal", None, None);
                                self.is_home_active = false;
                            }
                        });
                    });

            if let Some(idx) = switch_to_tab {
                self.is_home_active = false;
                self.active_tab_idx = idx;
                self.selected_sidebar_session_id = self.open_tabs.get(idx).and_then(|t| t.session_def.as_ref()).map(|def| def.id.clone());
                self.sync_active_tab_sftp();
                ctx.memory_mut(|m| {
                    if let Some(fid) = m.focused() {
                        m.surrender_focus(fid);
                    }
                });
            }

            // 2. QUICK CONNECT AREA (Visible on terminal tabs, hidden on Home Dashboard to avoid duplicate)
            let mut quick_ui = ui.new_child(egui::UiBuilder::new().max_rect(quick_rect).layout(Layout::right_to_left(Align::Center)));
            if !self.is_home_active {
                let run_btn = egui::Button::new(RichText::new("▶ Connect").color(Color32::WHITE).size(11.5).strong())
                    .fill(Color32::from_rgb(37, 99, 235))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)))
                    .rounding(5.0);
                if quick_ui.add(run_btn).on_hover_text("Quick Connect to user@host:port or local shell").clicked() {
                    self.handle_quick_connect();
                }

                egui::Frame::none()
                    .fill(Color32::from_rgb(16, 21, 30))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(32, 42, 60)))
                    .rounding(5.0)
                    .inner_margin(egui::Margin::symmetric(8.0, 2.0))
                    .show(&mut quick_ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⚡").color(Color32::from_rgb(56, 189, 248)).size(11.0));
                            let q_edit = ui.add_sized(
                                egui::vec2(140.0, 18.0),
                                TextEdit::singleline(&mut self.quick_connect_input)
                                    .hint_text("user@host:port...")
                                    .frame(false)
                            );
                            if q_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                self.handle_quick_connect();
                            }
                        });
                    });
            } else {
                quick_ui.label(
                    RichText::new("⚡ Modern Remote Toolbox")
                        .size(11.0)
                        .color(Color32::from_rgb(100, 116, 139)),
                );
            }

            // Tab Right-Click Context Menu (Floating Foreground Area)
            if let Some((menu_idx, pos)) = self.tab_context_menu {
                let mut close_menu = false;
                let menu_area = egui::Area::new(egui::Id::new("tab_context_menu_area"))
                    .fixed_pos(pos)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::menu(ui.style())
                            .fill(Color32::from_rgb(26, 32, 42))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(52, 60, 72)))
                            .rounding(6.0)
                            .shadow(egui::epaint::Shadow {
                                offset: vec2(0.0, 4.0),
                                blur: 8.0,
                                spread: 0.0,
                                color: Color32::from_black_alpha(160),
                            })
                            .inner_margin(egui::Margin::symmetric(6.0, 6.0))
                            .show(ui, |ui| {
                                ui.set_min_width(190.0);
                                if ui.button(RichText::new("🔑 Duplicate Tab").size(12.0).color(Color32::WHITE)).clicked() {
                                    tab_menu_action = Some((menu_idx, TabMenuAction::Duplicate));
                                    close_menu = true;
                                }
                                if ui.button(RichText::new("🔄 Restart / Reconnect").size(12.0).color(Color32::WHITE)).clicked() {
                                    tab_menu_action = Some((menu_idx, TabMenuAction::Restart));
                                    close_menu = true;
                                }
                                if ui.button(RichText::new("✏ Rename Tab").size(12.0).color(Color32::WHITE)).clicked() {
                                    tab_menu_action = Some((menu_idx, TabMenuAction::Rename));
                                    close_menu = true;
                                }
                                if ui.button(RichText::new("× Close Tab").size(12.0).color(Color32::from_rgb(248, 113, 113))).clicked() {
                                    tab_menu_action = Some((menu_idx, TabMenuAction::Close));
                                    close_menu = true;
                                }
                                ui.separator();
                                if ui.button(RichText::new("Close Other Tabs").size(12.0).color(Color32::from_rgb(210, 220, 230))).clicked() {
                                    tab_menu_action = Some((menu_idx, TabMenuAction::CloseOthers));
                                    close_menu = true;
                                }
                                if ui.button(RichText::new("Close Tabs to the Right").size(12.0).color(Color32::from_rgb(210, 220, 230))).clicked() {
                                    tab_menu_action = Some((menu_idx, TabMenuAction::CloseRight));
                                    close_menu = true;
                                }
                            });
                    });

                let clicked_outside = ctx.input(|i| i.pointer.primary_clicked() && !menu_area.response.rect.contains(i.pointer.interact_pos().unwrap_or(pos2(-100.0, -100.0))));
                let esc_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                if close_menu || clicked_outside || esc_pressed {
                    self.tab_context_menu = None;
                }
            }

                if let Some((idx, action)) = tab_menu_action {
                    match action {
                        TabMenuAction::Duplicate => {
                            let dup_info = self.open_tabs.get(idx).map(|tab| {
                                (
                                    tab.title.clone(),
                                    tab.session_def.clone(),
                                    tab.custom_cmd.clone(),
                                    tab.workdir.clone(),
                                )
                            });
                            if let Some((title, session_def, custom_cmd, workdir)) = dup_info {
                                if let Some(s) = session_def {
                                    self.spawn_ssh_terminal(s);
                                } else {
                                    let name = format!("{}_copy", title);
                                    self.spawn_local_terminal(&name, custom_cmd.as_deref(), workdir.as_deref());
                                }
                            }
                        }
                        TabMenuAction::Restart => {
                            self.restart_tab(idx);
                        }
                        TabMenuAction::Rename => {
                            if let Some(tab) = self.open_tabs.get(idx) {
                                self.rename_tab_target = Some((idx, tab.title.clone()));
                            }
                        }
                        TabMenuAction::Close => {
                            close_tab_idx = Some(idx);
                        }
                        TabMenuAction::CloseOthers => {
                            if idx < self.open_tabs.len() {
                                let keep = self.open_tabs.remove(idx);
                                for mut t in self.open_tabs.drain(..) {
                                    t.session.kill();
                                }
                                self.open_tabs.push(keep);
                                self.active_tab_idx = 0;
                            }
                        }
                        TabMenuAction::CloseRight => {
                            for mut t in self.open_tabs.drain(idx + 1..) {
                                t.session.kill();
                            }
                            if self.active_tab_idx > idx {
                                self.active_tab_idx = idx;
                            }
                        }
                    }
                }

                if let Some(close_idx) = close_tab_idx {
                    if close_idx < self.open_tabs.len() {
                        let mut closed = self.open_tabs.remove(close_idx);
                        closed.session.kill();
                        if self.open_tabs.is_empty() {
                            self.is_home_active = true;
                        } else if self.active_tab_idx >= self.open_tabs.len() {
                            self.active_tab_idx = self.open_tabs.len() - 1;
                        } else if self.active_tab_idx > close_idx {
                            self.active_tab_idx = self.active_tab_idx.saturating_sub(1);
                        }
                    }
                }

                ui.add_space(2.0);

            // If Home tab is active, render Modern Welcome Dashboard
            if self.is_home_active {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("⚡ RustyBox").size(30.0).strong().color(Color32::from_rgb(56, 189, 248)));
                    ui.label(RichText::new("High-Performance Remote Toolbox & Multi-Terminal Workstation").size(13.0).color(Color32::from_rgb(148, 163, 184)));
                    ui.add_space(18.0);

                    // Hero Quick Connect Box
                    egui::Frame::none()
                        .fill(Color32::from_rgb(16, 21, 30))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(32, 42, 60)))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(18.0, 12.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("⚡ Quick Connect:").strong().color(Color32::from_rgb(56, 189, 248)).size(13.0));
                                let q_edit = ui.add(
                                    TextEdit::singleline(&mut self.quick_connect_input)
                                        .hint_text("user@host:port, cmd, wsl...")
                                        .desired_width(340.0),
                                );
                                let conn_btn = egui::Button::new(
                                    RichText::new("▶ Connect").strong().color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(37, 99, 235))
                                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(96, 165, 250)))
                                .rounding(6.0);
                                if (q_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                    || ui.add(conn_btn).clicked()
                                {
                                    self.handle_quick_connect();
                                }
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let chip = |ui: &mut egui::Ui, text: &str| -> egui::Response {
                                    ui.add(
                                        egui::Button::new(
                                            RichText::new(text).size(11.0).color(Color32::from_rgb(190, 205, 225)),
                                        )
                                        .fill(Color32::from_rgb(22, 28, 40))
                                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(36, 46, 64)))
                                        .rounding(6.0),
                                    )
                                };
                                if chip(ui, "💻 Local Shell").clicked() {
                                    self.spawn_local_terminal("Local Terminal", None, None);
                                    self.is_home_active = false;
                                }
                                if chip(ui, "🐧 WSL").clicked() {
                                    #[cfg(target_os = "windows")]
                                    self.spawn_local_terminal("WSL", Some("wsl"), None);
                                    #[cfg(not(target_os = "windows"))]
                                    self.spawn_local_terminal("Local Shell", None, None);
                                    self.is_home_active = false;
                                }
                                if chip(ui, "🌐 New Session").clicked() {
                                    self.new_session_dialog.open_for_new();
                                }
                                if chip(ui, "📡 NetScan").clicked() {
                                    self.show_network_tools = true;
                                    self.net_tools_tab = 1;
                                }
                                if chip(ui, "🔀 Tunnels").clicked() {
                                    self.show_tunnels = true;
                                }
                            });
                        });

                    ui.add_space(22.0);

                    // 6 Modern Action Cards with icon badge & refined hover effects in a balanced 3x2 grid
                    let modern_card = |ui: &mut egui::Ui, icon: &str, title: &str, desc: &str, tint: Color32, w: f32, h: f32| -> egui::Response {
                        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::click());
                        let is_hovered = resp.hovered();
                        let bg = if is_hovered {
                            Color32::from_rgb(25, 34, 50)
                        } else {
                            Color32::from_rgb(17, 22, 32)
                        };
                        let border = if is_hovered {
                            egui::Stroke::new(1.2, Color32::from_rgb(56, 189, 248))
                        } else {
                            egui::Stroke::new(1.0, Color32::from_rgb(32, 42, 60))
                        };
                        ui.painter().rect(rect, egui::Rounding::same(8.0), bg, border);

                        ui.allocate_new_ui(
                            egui::UiBuilder::new()
                                .max_rect(rect.shrink(8.0))
                                .layout(Layout::left_to_right(Align::Center)),
                            |c_ui| {
                                // Icon badge container
                                let (badge_rect, _) = c_ui.allocate_exact_size(vec2(38.0, 38.0), egui::Sense::hover());
                                c_ui.painter().rect_filled(badge_rect, 6.0, Color32::from_rgb(24, 32, 46));
                                c_ui.painter().rect_stroke(badge_rect, 6.0, egui::Stroke::new(1.0, tint));
                                c_ui.painter().text(
                                    badge_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    icon,
                                    egui::FontId::proportional(20.0),
                                    Color32::WHITE,
                                );

                                c_ui.add_space(8.0);
                                c_ui.vertical(|ui| {
                                    let tc = if is_hovered {
                                        Color32::from_rgb(56, 189, 248)
                                    } else {
                                        Color32::WHITE
                                    };
                                    ui.label(RichText::new(title).strong().size(12.5).color(tc));
                                    ui.label(RichText::new(desc).size(10.0).color(Color32::from_rgb(148, 163, 184)));
                                });
                            },
                        );
                        resp
                    };

                    let card_w = 236.0;
                    let card_h = 74.0;
                    let card_spacing = 12.0;

                    // Row 1: Local Terminal, SSH Client, SFTP Browser
                    ui.horizontal(|ui| {
                        if modern_card(ui, "💻", "Local Terminal", "Interactive shell with ConPTY", Color32::from_rgb(56, 189, 248), card_w, card_h).clicked() {
                            self.spawn_local_terminal("Local Terminal", None, None);
                            self.is_home_active = false;
                        }
                        ui.add_space(card_spacing);
                        if modern_card(ui, "🌐", "SSH Client", "Key auth & auto-SFTP stream", Color32::from_rgb(96, 165, 250), card_w, card_h).clicked() {
                            self.new_session_dialog.open_for_new();
                        }
                        ui.add_space(card_spacing);
                        if modern_card(ui, "📁", "SFTP Browser", "Dual-pane remote file manager", Color32::from_rgb(52, 211, 153), card_w, card_h).clicked() {
                            self.show_sftp_sidebar = true;
                            self.sidebar_tab = 1;
                        }
                    });

                    ui.add_space(card_spacing);

                    // Row 2: Network Scanner, SSH Tunnels, Network Daemons
                    ui.horizontal(|ui| {
                        if modern_card(ui, "📡", "Network Scanner", "Subnet IP discovery & Port Scan", Color32::from_rgb(251, 191, 36), card_w, card_h).clicked() {
                            self.show_network_tools = true;
                            self.net_tools_tab = 1;
                        }
                        ui.add_space(card_spacing);
                        if modern_card(ui, "🔀", "SSH Tunnels", "Local, remote & SOCKS5 proxy", Color32::from_rgb(192, 132, 252), card_w, card_h).clicked() {
                            self.show_tunnels = true;
                        }
                        ui.add_space(card_spacing);
                        if modern_card(ui, "🕊", "Network Daemons", "Micro HTTP, TFTP & Syslog", Color32::from_rgb(244, 114, 182), card_w, card_h).clicked() {
                            self.show_services_modal = true;
                        }
                    });

                    ui.add_space(26.0);
                    ui.label(RichText::new("★ Quick Launch Sessions").strong().size(13.5).color(Color32::from_rgb(234, 179, 8)));
                    ui.add_space(10.0);

                    let mut to_launch_quick: Option<Session> = None;
                    let top_sessions: Vec<Session> = self.sessions.iter().take(6).cloned().collect();
                    for chunk in top_sessions.chunks(3) {
                        ui.horizontal(|ui| {
                            for (i, s) in chunk.iter().enumerate() {
                                if i > 0 {
                                    ui.add_space(card_spacing);
                                }
                                let icon = match s.session_type {
                                    SessionType::LocalShell { .. } => "💻",
                                    SessionType::Wsl { .. } => "🐧",
                                    _ => "🔑",
                                };
                                let (q_rect, q_resp) = ui.allocate_exact_size(vec2(card_w, 36.0), egui::Sense::click());
                                let is_h = q_resp.hovered();
                                let bg = if is_h { Color32::from_rgb(26, 36, 54) } else { Color32::from_rgb(18, 23, 33) };
                                let border = if is_h { egui::Stroke::new(1.0, Color32::from_rgb(56, 189, 248)) } else { egui::Stroke::new(1.0, Color32::from_rgb(32, 42, 60)) };
                                ui.painter().rect(q_rect, 6.0, bg, border);

                                ui.allocate_new_ui(
                                    egui::UiBuilder::new()
                                        .max_rect(q_rect.shrink(6.0))
                                        .layout(Layout::left_to_right(Align::Center)),
                                    |c_ui| {
                                        c_ui.label(RichText::new(icon).size(13.0));
                                        c_ui.add_space(4.0);
                                        let title_col = if is_h { Color32::from_rgb(56, 189, 248) } else { Color32::WHITE };
                                        c_ui.label(RichText::new(&s.name).strong().size(11.5).color(title_col));
                                    },
                                );
                                if q_resp.clicked() {
                                    to_launch_quick = Some(s.clone());
                                }
                            }
                        });
                        ui.add_space(8.0);
                    }
                    if let Some(s) = to_launch_quick {
                        self.launch_generic_session(s);
                        self.is_home_active = false;
                    }
                });
                return;
            }

            // INLINE TERMINAL SEARCH BAR (Ctrl+F)
            if self.search_open {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔍 Find:").strong().color(Color32::from_rgb(255, 200, 80)));
                    let text_resp = ui.add(TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search across scrollback & screen...")
                        .desired_width(240.0));
                    if self.focus_search {
                        text_resp.request_focus();
                        self.focus_search = false;
                    }

                    // Search across all 2,000 scrollback lines + visible screen if active tab exists
                    if !self.search_query.is_empty() {
                        let nav_idx_id = egui::Id::new("term_search_match_idx").with(self.active_tab_idx);
                        let mut matches = Vec::new();
                        if let Some(tab) = self.open_tabs.get(self.active_tab_idx) {
                            let mut parser = tab.session.parser.lock();
                            matches = crate::terminal::widget::search_terminal_buffer(
                                parser.screen_mut(),
                                &self.search_query,
                            );
                        }
                        let match_count = matches.len();
                        let mut cur_match_idx: usize = ui
                            .data(|d| d.get_temp(nav_idx_id))
                            .unwrap_or_else(|| match_count.saturating_sub(1));
                        if match_count > 0 && cur_match_idx >= match_count {
                            cur_match_idx = match_count - 1;
                        }

                        if match_count > 0 {
                            ui.label(
                                RichText::new(format!(
                                    "{} / {} match{}",
                                    cur_match_idx + 1,
                                    match_count,
                                    if match_count > 1 { "es" } else { "" }
                                ))
                                .color(Color32::from_rgb(100, 220, 100))
                                .strong(),
                            );

                            let enter_pressed = text_resp.has_focus()
                                && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                            let shift_held = ctx.input(|i| i.modifiers.shift);
                            let mut jump_to: Option<usize> = None;

                            if ui.small_button("▲ Prev").clicked() || (enter_pressed && shift_held) {
                                cur_match_idx = if cur_match_idx == 0 {
                                    match_count - 1
                                } else {
                                    cur_match_idx - 1
                                };
                                jump_to = Some(cur_match_idx);
                            }
                            if ui.small_button("▼ Next").clicked() || (enter_pressed && !shift_held) {
                                cur_match_idx = (cur_match_idx + 1) % match_count;
                                jump_to = Some(cur_match_idx);
                            }

                            if let Some(target_idx) = jump_to {
                                ui.data_mut(|d| d.insert_temp(nav_idx_id, target_idx));
                                if let Some((abs_row, _, _)) = matches.get(target_idx) {
                                    if let Some(tab) = self.open_tabs.get(self.active_tab_idx) {
                                        let mut parser = tab.session.parser.lock();
                                        crate::terminal::widget::scroll_to_abs_row(
                                            parser.screen_mut(),
                                            *abs_row,
                                        );
                                    }
                                }
                            }
                        } else {
                            ui.label(RichText::new("0 matches").color(Color32::from_rgb(200, 100, 100)));
                        }

                        if ui.small_button("Clear").clicked() {
                            self.search_query.clear();
                        }
                    }

                    if ui.small_button("✖ Close (Esc)").clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.search_open = false;
                        self.search_query.clear();
                    }
                });
                ui.separator();
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
            let current_active_idx = self.active_tab_idx.min(num_tabs - 1);
            let mut newly_focused_tab_idx: Option<usize> = None;

            let render_pane = |ui: &mut egui::Ui,
                               pane_idx: usize,
                               tab: &mut OpenTab,
                               is_active: bool|
             -> (egui::Response, TerminalAction) {
                let eff_theme = tab
                    .session_def
                    .as_ref()
                    .and_then(|s| TerminalTheme::from_name(&s.terminal_color_scheme))
                    .unwrap_or(current_theme);
                let eff_font = tab
                    .session_def
                    .as_ref()
                    .map(|s| s.terminal_font_size as f32)
                    .filter(|&sz| sz >= 9.0 && (font_sz - 14.0).abs() < 0.1)
                    .unwrap_or(font_sz);
                let bs_ctrl_h = tab
                    .session_def
                    .as_ref()
                    .map(|s| s.backspace_sends_ctrl_h)
                    .unwrap_or(false);

                let widget = TerminalWidget::new(&tab.id, &mut tab.session)
                    .with_font_size(eff_font)
                    .with_theme(eff_theme)
                    .with_search(search_ref)
                    .with_active_pane(is_active)
                    .with_backspace_ctrl_h(bs_ctrl_h);
                let _ = pane_idx;
                widget.ui(ui, broadcast_tx)
            };

            match self.split_mode {
                SplitMode::Single => {
                    let tab = &mut self.open_tabs[current_active_idx];
                    let (_, act) = render_pane(ui, current_active_idx, tab, true);
                    triggered_action = act;
                }
                SplitMode::Horizontal => {
                    let tab_a_idx = current_active_idx;
                    let tab_b_idx = if num_tabs > 1 {
                        if tab_a_idx == 0 { 1 } else { 0 }
                    } else {
                        tab_a_idx
                    };

                    ui.columns(2, |columns| {
                        let (resp_a, act_a) = {
                            let tab_a = &mut self.open_tabs[tab_a_idx];
                            render_pane(&mut columns[0], tab_a_idx, tab_a, true)
                        };
                        if resp_a.clicked() || resp_a.drag_started() || resp_a.secondary_clicked() {
                            newly_focused_tab_idx = Some(tab_a_idx);
                        }
                        if !matches!(act_a, TerminalAction::None) {
                            newly_focused_tab_idx = Some(tab_a_idx);
                            triggered_action = act_a;
                        }

                        if tab_a_idx != tab_b_idx {
                            let (resp_b, act_b) = {
                                let tab_b = &mut self.open_tabs[tab_b_idx];
                                render_pane(&mut columns[1], tab_b_idx, tab_b, false)
                            };
                            if resp_b.clicked() || resp_b.drag_started() || resp_b.secondary_clicked() {
                                newly_focused_tab_idx = Some(tab_b_idx);
                            }
                            if !matches!(act_b, TerminalAction::None) {
                                newly_focused_tab_idx = Some(tab_b_idx);
                                if matches!(triggered_action, TerminalAction::None) {
                                    triggered_action = act_b;
                                }
                            }
                        } else {
                            columns[1].centered_and_justified(|ui| {
                                ui.label("Open another session to view side-by-side");
                            });
                        }
                    });
                }
                SplitMode::Vertical => {
                    let tab_a_idx = current_active_idx;
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
                                let (resp_a, act_a) = render_pane(ui, tab_a_idx, tab_a, true);
                                if resp_a.clicked() || resp_a.drag_started() || resp_a.secondary_clicked() {
                                    newly_focused_tab_idx = Some(tab_a_idx);
                                }
                                if !matches!(act_a, TerminalAction::None) {
                                    newly_focused_tab_idx = Some(tab_a_idx);
                                    triggered_action = act_a;
                                }
                            },
                        );

                        ui.separator();

                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), half_height),
                            Layout::top_down(Align::Min),
                            |ui| {
                                if tab_a_idx != tab_b_idx {
                                    let tab_b = &mut self.open_tabs[tab_b_idx];
                                    let (resp_b, act_b) = render_pane(ui, tab_b_idx, tab_b, false);
                                    if resp_b.clicked() || resp_b.drag_started() || resp_b.secondary_clicked() {
                                        newly_focused_tab_idx = Some(tab_b_idx);
                                    }
                                    if !matches!(act_b, TerminalAction::None) {
                                        newly_focused_tab_idx = Some(tab_b_idx);
                                        if matches!(triggered_action, TerminalAction::None) {
                                            triggered_action = act_b;
                                        }
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
                                    let (resp0, act0) =
                                        render_pane(&mut cols[0], 0, tab0, current_active_idx == 0);
                                    if resp0.clicked() || resp0.drag_started() || resp0.secondary_clicked() {
                                        newly_focused_tab_idx = Some(0);
                                    }
                                    if !matches!(act0, TerminalAction::None) {
                                        newly_focused_tab_idx = Some(0);
                                        triggered_action = act0;
                                    }
                                }
                                if num_tabs > 1 {
                                    let tab1 = &mut self.open_tabs[1];
                                    let (resp1, act1) =
                                        render_pane(&mut cols[1], 1, tab1, current_active_idx == 1);
                                    if resp1.clicked() || resp1.drag_started() || resp1.secondary_clicked() {
                                        newly_focused_tab_idx = Some(1);
                                    }
                                    if !matches!(act1, TerminalAction::None) {
                                        newly_focused_tab_idx = Some(1);
                                        if matches!(triggered_action, TerminalAction::None) {
                                            triggered_action = act1;
                                        }
                                    }
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
                                    let (resp2, act2) =
                                        render_pane(&mut cols[0], 2, tab2, current_active_idx == 2);
                                    if resp2.clicked() || resp2.drag_started() || resp2.secondary_clicked() {
                                        newly_focused_tab_idx = Some(2);
                                    }
                                    if !matches!(act2, TerminalAction::None) {
                                        newly_focused_tab_idx = Some(2);
                                        if matches!(triggered_action, TerminalAction::None) {
                                            triggered_action = act2;
                                        }
                                    }
                                }
                                if num_tabs > 3 {
                                    let tab3 = &mut self.open_tabs[3];
                                    let (resp3, act3) =
                                        render_pane(&mut cols[1], 3, tab3, current_active_idx == 3);
                                    if resp3.clicked() || resp3.drag_started() || resp3.secondary_clicked() {
                                        newly_focused_tab_idx = Some(3);
                                    }
                                    if !matches!(act3, TerminalAction::None) {
                                        newly_focused_tab_idx = Some(3);
                                        if matches!(triggered_action, TerminalAction::None) {
                                            triggered_action = act3;
                                        }
                                    }
                                }
                            });
                        },
                    );
                }
            }

            if let Some(new_idx) = newly_focused_tab_idx {
                self.active_tab_idx = new_idx;
            }
        });

        // Handle Action from Terminal
        match triggered_action {
            TerminalAction::MultiLinePaste(text) => {
                if self.paste_protection_enabled {
                    self.pending_paste_text = Some(text);
                } else if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
                    if let Ok(bytes) = tab.session.paste_text(&text) {
                        if self.multi_exec_enabled {
                            let _ = self.broadcast_tx.send(bytes);
                        }
                    }
                }
            }
            TerminalAction::SaveBufferToFile => {
                self.save_active_terminal_buffer();
            }
            TerminalAction::ZoomIn => {
                self.font_size = (self.font_size + 1.0).min(32.0);
            }
            TerminalAction::ZoomOut => {
                self.font_size = (self.font_size - 1.0).max(9.0);
            }
            TerminalAction::ZoomReset => {
                self.font_size = 14.0;
            }
            TerminalAction::OpenSearch => {
                self.search_open = true;
                self.focus_search = true;
            }
            TerminalAction::RestartTab => {
                let active = self.active_tab_idx;
                self.restart_tab(active);
            }
            TerminalAction::CloseTab => {
                if !self.open_tabs.is_empty() {
                    let active = self.active_tab_idx;
                    if active < self.open_tabs.len() {
                        let mut closed = self.open_tabs.remove(active);
                        closed.session.kill();
                        if self.open_tabs.is_empty() {
                            self.is_home_active = true;
                        } else if self.active_tab_idx >= self.open_tabs.len() {
                            self.active_tab_idx = self.open_tabs.len() - 1;
                        }
                    }
                }
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
                    if let Some(tab) = self.open_tabs.get_mut(self.active_tab_idx) {
                        if let Ok(bytes) = tab.session.paste_text(&text) {
                            if self.multi_exec_enabled {
                                let _ = self.broadcast_tx.send(bytes);
                            }
                        }
                    }
                }
            } else if close {
                self.pending_paste_text = None;
            }
        }

        // New Session Dialog
        self.new_session_dialog.available_credentials = self.credentials.clone();
        if let Some(new_session) = render_new_session_dialog(ctx, &mut self.new_session_dialog) {
            if let Some(idx) = self.sessions.iter().position(|s| s.id == new_session.id) {
                // Editing existing session: update in-place
                self.sessions[idx] = new_session.clone();
            } else {
                // Brand new session: append to list
                self.sessions.push(new_session.clone());
            }
            let _ = self.storage.save_sessions(&self.sessions);
            self.selected_sidebar_session_id = Some(new_session.id.clone());
            self.is_home_active = false;
            self.launch_generic_session(new_session);
        }

        if self.new_session_dialog.request_open_passwords {
            self.new_session_dialog.request_open_passwords = false;
            self.manage_passwords_tab = 1; // Open Credentials / Users tab directly
            self.show_manage_passwords = true;
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
                        ui.selectable_value(&mut self.net_tools_tab, 3, "📊 Netstat");
                        ui.selectable_value(&mut self.net_tools_tab, 4, "🌐 DNS Lookup");
                        ui.selectable_value(&mut self.net_tools_tab, 5, "⚡ Wake-on-LAN");
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
                        4 => {
                            // DNS Lookup
                            ui.label("Domain Name System (DNS) & Hostname Resolution:");
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("Hostname / Domain:");
                                ui.add(TextEdit::singleline(&mut self.network_tools.dns_query).desired_width(220.0).hint_text("e.g. google.com"));
                                if ui.button("🔍 Resolve IP").clicked() {
                                    self.network_tools.lookup_dns();
                                }
                            });
                            ui.separator();
                            ui.label(RichText::new("Resolution Results:").strong());
                            ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                                if self.network_tools.dns_results.is_empty() {
                                    ui.label("Enter a hostname and click Resolve IP.");
                                } else {
                                    for res in &self.network_tools.dns_results {
                                        ui.label(RichText::new(res).monospace().color(Color32::from_rgb(56, 189, 248)));
                                    }
                                }
                            });
                        }
                        5 => {
                            // Wake-on-LAN
                            ui.label("Send Wake-on-LAN (WOL) Magic Packets to power on remote computers:");
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("MAC Address:");
                                ui.add(TextEdit::singleline(&mut self.network_tools.wol_mac).desired_width(180.0).hint_text("00:11:22:33:44:55"));
                                ui.label("UDP Port:");
                                ui.add(egui::DragValue::new(&mut self.network_tools.wol_port).range(1..=65535));
                            });
                            ui.add_space(4.0);
                            if ui.button(RichText::new("⚡ Send Magic Packet").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                                self.network_tools.send_wol();
                            }
                            if let Some(ref st) = self.network_tools.wol_status {
                                ui.add_space(4.0);
                                ui.label(RichText::new(st).color(Color32::from_rgb(80, 240, 120)).strong());
                            }
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
                        ui.selectable_value(&mut self.services_tab, 0, "🌐 HTTP Server");
                        ui.selectable_value(&mut self.services_tab, 1, "📡 Syslog Server");
                        ui.selectable_value(&mut self.services_tab, 2, "📁 TFTP Server");
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
                    } else if self.services_tab == 1 {
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
                    } else {
                        // TFTP Server
                        ui.label("Embedded Micro TFTP Server (RFC 1350) for router/firmware backups:");
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("TFTP Root Folder:");
                            let path_str = self.network_tools.tftp_root.to_string_lossy().to_string();
                            ui.label(RichText::new(path_str).monospace().color(Color32::from_rgb(100, 200, 255)));
                            if ui.button("Browse...").clicked() {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.network_tools.tftp_root = folder;
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("TFTP UDP Port:");
                            ui.add(egui::DragValue::new(&mut self.network_tools.tftp_port).range(1..=65535));

                            let is_running = self.network_tools.tftp_running.load(std::sync::atomic::Ordering::Relaxed);
                            if is_running {
                                if ui.button(RichText::new("⏹ Stop TFTP").color(Color32::from_rgb(255, 100, 100))).clicked() {
                                    self.network_tools.stop_tftp_server();
                                }
                                ui.label(RichText::new(format!("🟢 Listening on UDP port {}", self.network_tools.tftp_port)).color(Color32::from_rgb(80, 240, 120)));
                            } else {
                                if ui.button(RichText::new("▶ Start TFTP").color(Color32::from_rgb(80, 240, 120))).clicked() {
                                    self.network_tools.start_tftp_server(&rt_handle);
                                }
                                ui.label(RichText::new("⚪ Offline").weak());
                            }
                        });

                        ui.separator();
                        ui.label(RichText::new("TFTP Request & Transfer Log:").strong());
                        ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            let logs = self.network_tools.tftp_logs.lock().clone();
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
            let mut tunnel_to_remove: Option<usize> = None;

            egui::Window::new("🔀 SSH Tunnels / Port Forwarding")
                .open(&mut open)
                .default_size(Vec2::new(660.0, 420.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Configured SSH Port Forwarding Tunnels:").strong());
                    ui.add_space(4.0);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(17, 21, 30))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(45, 53, 72)))
                        .rounding(3.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            egui::Grid::new("tunnels_grid")
                                .striped(true)
                                .min_col_width(70.0)
                                .spacing(vec2(12.0, 6.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Status").strong());
                                    ui.label(RichText::new("Tunnel Name").strong());
                                    ui.label(RichText::new("Local Port").strong());
                                    ui.label(RichText::new("Remote Target").strong());
                                    ui.label(RichText::new("SSH Gateway").strong());
                                    ui.label(RichText::new("Actions").strong());
                                    ui.end_row();

                                    for (idx, tunnel) in self.tunnel_manager.tunnels.iter().enumerate() {
                                        let is_running = self.tunnel_manager.active_states[idx]
                                            .load(std::sync::atomic::Ordering::Relaxed);

                                        if is_running {
                                            ui.label(RichText::new("🟢 Running").color(Color32::from_rgb(46, 160, 67)).strong());
                                        } else {
                                            ui.label(RichText::new("⚪ Stopped").weak());
                                        }

                                        ui.label(&tunnel.name);
                                        ui.label(format!(":{}", tunnel.local_port));
                                        ui.label(format!("{}:{}", tunnel.remote_host, tunnel.remote_port));
                                        ui.label(&tunnel.ssh_server);

                                        ui.horizontal(|ui| {
                                            if is_running {
                                                if ui.small_button("⏹ Stop").clicked() {
                                                    self.tunnel_manager.stop_tunnel(idx);
                                                }
                                            } else {
                                                if ui.small_button("▶ Start").clicked() {
                                                    self.tunnel_manager.start_tunnel(idx, &rt_handle);
                                                }
                                            }
                                            if ui.small_button("🗑").on_hover_text("Delete Tunnel").clicked() {
                                                tunnel_to_remove = Some(idx);
                                            }
                                        });
                                        ui.end_row();
                                    }
                                });
                        });

                    if let Some(idx) = tunnel_to_remove {
                        self.tunnel_manager.remove_tunnel(idx);
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("➕ New SSH tunnel").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                            self.show_tunnel_edit = true;
                        }
                    });
                });
            self.show_tunnels = open;
        }

        // New SSH tunnel creation dialog (TFormPortForwardingEdit)
        if self.show_tunnel_edit {
            let mut open = self.show_tunnel_edit;
            let mut close_te = false;
            let mut save_te = false;
            egui::Window::new("New SSH tunnel")
                .open(&mut open)
                .default_size(Vec2::new(580.0, 360.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.tunnel_edit_mode, 0, "Local port forwarding");
                        ui.radio_value(&mut self.tunnel_edit_mode, 1, "Remote port forwarding");
                        ui.radio_value(&mut self.tunnel_edit_mode, 2, "Dynamic (SOCKS proxy)");
                    });
                    ui.separator();
                    ui.add_space(4.0);
                    let desc = match self.tunnel_edit_mode {
                        0 => "Local clients ──(127.0.0.1:ForwardedPort)──► SSH Server ──► Remote Server:RemotePort",
                        1 => "Remote clients ──(SSH Server:ForwardedPort)──► SSH Tunnel ──► Local Target:LocalPort",
                        _ => "Local apps (SOCKS5 127.0.0.1:ForwardedPort) ──► SSH Server ──► Any Remote Host",
                    };
                    ui.label(RichText::new(desc).monospace().size(11.5).color(Color32::from_rgb(56, 189, 248)));
                    ui.add_space(6.0);

                    egui::Grid::new("tunnel_edit_grid").num_columns(4).spacing([10.0, 8.0]).show(ui, |ui| {
                        ui.label("Tunnel name:");
                        ui.add(TextEdit::singleline(&mut self.new_tunnel_name).desired_width(170.0));
                        ui.label("Forwarded port:");
                        ui.add(egui::DragValue::new(&mut self.new_tunnel_local_port).range(1..=65535));
                        ui.end_row();

                        ui.label("SSH server:");
                        ui.add(TextEdit::singleline(&mut self.tunnel_ssh_srv).desired_width(170.0));
                        ui.label("SSH port:");
                        ui.add(egui::DragValue::new(&mut self.tunnel_ssh_port).range(1..=65535));
                        ui.end_row();

                        ui.label("SSH login:");
                        ui.add(TextEdit::singleline(&mut self.tunnel_ssh_user).desired_width(170.0));
                        ui.label("");
                        ui.label("");
                        ui.end_row();

                        if self.tunnel_edit_mode != 2 {
                            ui.label("Remote server:");
                            ui.add(TextEdit::singleline(&mut self.new_tunnel_remote_host).desired_width(170.0));
                            ui.label("Remote port:");
                            ui.add(egui::DragValue::new(&mut self.new_tunnel_remote_port).range(1..=65535));
                            ui.end_row();
                        }
                    });
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("💾 Save Tunnel").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                            save_te = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_te = true;
                        }
                    });
                });
            if save_te {
                let name = if self.new_tunnel_name.trim().is_empty() {
                    format!("Tunnel :{}", self.new_tunnel_local_port)
                } else {
                    self.new_tunnel_name.trim().to_string()
                };
                let mode = match self.tunnel_edit_mode {
                    2 => crate::tunneling::TunnelMode::DynamicSocks5,
                    _ => crate::tunneling::TunnelMode::LocalForward,
                };
                let new_cfg = crate::tunneling::TunnelConfig {
                    id: uuid::Uuid::new_v4().to_string(),
                    name,
                    mode,
                    local_port: self.new_tunnel_local_port,
                    remote_host: self.new_tunnel_remote_host.trim().to_string(),
                    remote_port: self.new_tunnel_remote_port,
                    ssh_server: format!("{}@{}:{}", self.tunnel_ssh_user, self.tunnel_ssh_srv, self.tunnel_ssh_port),
                };
                self.tunnel_manager.add_tunnel(new_cfg);
                self.new_tunnel_name.clear();
                close_te = true;
            }
            self.show_tunnel_edit = open && !close_te;
        }

        // KEYGEN DIALOG (RustyBox KeyGen)
        if self.show_keygen {
            let mut open = self.show_keygen;
            let mut close_kg = false;
            egui::Window::new("🔑 RustyBox KeyGen - SSH Key Generator")
                .open(&mut open)
                .default_size(Vec2::new(560.0, 440.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Generate OpenSSH Public / Private Key Pairs").strong());
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Key Type:");
                        ui.radio_value(&mut self.keygen_type, 0, "Ed25519 (Modern & Fast)");
                        ui.radio_value(&mut self.keygen_type, 1, "RSA (4096-bit Standard)");
                    });

                    ui.horizontal(|ui| {
                        ui.label("Key Comment:");
                        ui.text_edit_singleline(&mut self.keygen_comment);
                    });

                    ui.add_space(4.0);
                    if ui.button(RichText::new("⚡ Generate New Key Pair").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                        self.generate_ssh_key_pair();
                    }

                    if !self.keygen_public.is_empty() {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Public Key (authorized_keys format):").strong());
                            if ui.button("📋 Copy").clicked() {
                                ctx.copy_text(self.keygen_public.clone());
                            }
                        });
                        ui.add(egui::TextEdit::multiline(&mut self.keygen_public).desired_rows(3).font(egui::TextStyle::Monospace));

                        if !self.keygen_fingerprint.is_empty() {
                            ui.label(format!("Key Fingerprint: {}", self.keygen_fingerprint));
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.button("💾 Save Public Key (.pub)...").clicked() {
                                if let Some(path) = rfd::FileDialog::new().add_filter("Public Key", &["pub"]).save_file() {
                                    let _ = std::fs::write(&path, &self.keygen_public);
                                }
                            }
                            if ui.button("🔒 Save Private Key...").clicked() {
                                if let Some(path) = rfd::FileDialog::new().save_file() {
                                    let _ = std::fs::write(&path, &self.keygen_private);
                                }
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    if ui.button("Close").clicked() {
                        close_kg = true;
                    }
                });
            self.show_keygen = open && !close_kg;
        }

        // RANDOM PASSWORD GENERATOR
        if self.show_pwdgen {
            let mut open = self.show_pwdgen;
            let mut close_pg = false;
            egui::Window::new("🗝 Random Password Generator")
                .open(&mut open)
                .default_size(Vec2::new(480.0, 320.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Generate Cryptographically Strong Passwords").strong());
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Password Length:");
                        ui.add(egui::DragValue::new(&mut self.pwdgen_len).range(8..=64));
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.pwdgen_upper, "Uppercase (A-Z)");
                        ui.checkbox(&mut self.pwdgen_lower, "Lowercase (a-z)");
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.pwdgen_digits, "Numbers (0-9)");
                        ui.checkbox(&mut self.pwdgen_symbols, "Symbols (!@#$%)");
                    });

                    ui.add_space(6.0);
                    if ui.button(RichText::new("⚡ Generate Password").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                        self.generate_random_password();
                    }

                    if !self.pwdgen_output.is_empty() {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.pwdgen_output).font(egui::TextStyle::Monospace).desired_width(280.0));
                            if ui.button("📋 Copy").clicked() {
                                ctx.copy_text(self.pwdgen_output.clone());
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    if ui.button("Close").clicked() {
                        close_pg = true;
                    }
                });
            self.show_pwdgen = open && !close_pg;
        }

        // FILE COMPARISON (RustyBox Diff)
        if self.show_diff {
            let mut open = self.show_diff;
            let mut close_df = false;
            egui::Window::new("📄 File Comparison (RustyBox Diff)")
                .open(&mut open)
                .default_size(Vec2::new(660.0, 480.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new("Visual Text & Configuration Diff Tool").strong());
                    ui.separator();

                    ui.columns(2, |cols| {
                        cols[0].label(RichText::new("Original Text / File A:").strong());
                        cols[0].add(egui::TextEdit::multiline(&mut self.diff_text_a).desired_rows(8).font(egui::TextStyle::Monospace));
                        if cols[0].button("Load File A...").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                if let Ok(s) = std::fs::read_to_string(&path) {
                                    self.diff_text_a = s;
                                }
                            }
                        }

                        cols[1].label(RichText::new("Modified Text / File B:").strong());
                        cols[1].add(egui::TextEdit::multiline(&mut self.diff_text_b).desired_rows(8).font(egui::TextStyle::Monospace));
                        if cols[1].button("Load File B...").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                if let Ok(s) = std::fs::read_to_string(&path) {
                                    self.diff_text_b = s;
                                }
                            }
                        }
                    });

                    ui.add_space(6.0);
                    if ui.button(RichText::new("🔍 Compare Texts / Files").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                        self.compute_diff();
                    }

                    if !self.diff_result.is_empty() {
                        ui.separator();
                        ui.label(RichText::new("Diff Results (- removed, + added):").strong());
                        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                            for line in &self.diff_result {
                                if line.starts_with('+') {
                                    ui.label(RichText::new(line).color(Color32::from_rgb(46, 160, 67)).monospace());
                                } else if line.starts_with('-') {
                                    ui.label(RichText::new(line).color(Color32::from_rgb(248, 81, 73)).monospace());
                                } else {
                                    ui.label(RichText::new(line).monospace());
                                }
                            }
                        });
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    if ui.button("Close").clicked() {
                        close_df = true;
                    }
                });
            self.show_diff = open && !close_df;
        }

        // RUSTYBOX APT PACKAGE MANAGER (TFormMobApt)
        if self.show_mobapt {
            let mut open = self.show_mobapt;
            let mut close_apt = false;
            let packages = [
                ("openssh", "9.8p1", "Secure Shell client and key utilities", true),
                ("curl", "8.9.1", "Command line tool for transferring data with URLs", true),
                ("git", "2.46.0", "Fast, scalable, distributed revision control system", true),
                ("tmux", "3.4", "Terminal multiplexer", false),
                ("htop", "3.3.0", "Interactive processes viewer", true),
                ("nmap", "7.95", "Network exploration tool and security / port scanner", false),
                ("rsync", "3.3.0", "Fast, versatile, remote file-copying tool", true),
                ("jq", "1.7.1", "Lightweight and flexible command-line JSON processor", true),
                ("iperf3", "3.17", "TCP, UDP, and SCTP network bandwidth measurement tool", false),
                ("mtr", "0.95", "Network diagnostic tool combining ping and traceroute", false),
            ];

            egui::Window::new("📦 RustyBox Apt Package Manager")
                .open(&mut open)
                .default_size(Vec2::new(620.0, 440.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Filter packages:");
                        ui.add(TextEdit::singleline(&mut self.mobapt_search).desired_width(200.0).hint_text("curl, git, tmux, htop, nmap..."));
                    });
                    ui.separator();

                    ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        egui::Grid::new("apt_pkg_grid")
                            .striped(true)
                            .min_col_width(70.0)
                            .spacing(vec2(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("Package").strong());
                                ui.label(RichText::new("Version").strong());
                                ui.label(RichText::new("Description").strong());
                                ui.label(RichText::new("Status").strong());
                                ui.end_row();

                                for (pkg, ver, desc, installed) in &packages {
                                    if !self.mobapt_search.trim().is_empty()
                                        && !pkg.contains(&self.mobapt_search.to_lowercase())
                                        && !desc.to_lowercase().contains(&self.mobapt_search.to_lowercase())
                                    {
                                        continue;
                                    }

                                    ui.label(RichText::new(*pkg).strong());
                                    ui.label(*ver);
                                    ui.label(*desc);
                                    if *installed {
                                        ui.label(RichText::new("Installed").color(Color32::from_rgb(46, 160, 67)).strong());
                                    } else {
                                        ui.label(RichText::new("Available").color(Color32::from_rgb(56, 189, 248)));
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    if ui.button("Close").clicked() {
                        close_apt = true;
                    }
                });
            self.show_mobapt = open && !close_apt;
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
            let mut close_vault = false;
            let cfg = self.storage.load_config();
            let is_vault_configured = cfg.vault.is_some();
            let is_unlocked = self.storage.is_vault_unlocked();

            egui::Window::new("🔒 Master Password & AES-256 Vault")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(480.0, 320.0))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.heading(RichText::new("RustyBox Credentials Vault").color(Color32::from_rgb(56, 189, 248)));
                    ui.label("Military-grade AES-256-GCM encryption with Argon2id key derivation.");
                    ui.add_space(8.0);

                    if !is_vault_configured {
                        // Case A: No Master Password set yet
                        ui.group(|ui| {
                            ui.label(RichText::new("Status: Unencrypted (Plaintext)").color(Color32::from_rgb(248, 113, 113)).strong());
                            ui.label("Set a master password to encrypt all session and credential passwords in data/RustyBox.json.");
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                ui.label("New Master Password:   ");
                                ui.add(TextEdit::singleline(&mut self.master_password).password(true).desired_width(180.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Confirm Master Password:");
                                ui.add(TextEdit::singleline(&mut self.master_password_confirm).password(true).desired_width(180.0));
                            });

                            ui.add_space(10.0);
                            if ui.add(
                                egui::Button::new(RichText::new("🔐 Enable Master Password & Encrypt Data").strong().color(Color32::WHITE))
                                    .fill(Color32::from_rgb(34, 197, 94))
                            ).clicked()
                            {
                                if self.master_password.is_empty() {
                                    self.vault_status = Some(("❌ Master password cannot be empty!".into(), true));
                                } else if self.master_password != self.master_password_confirm {
                                    self.vault_status = Some(("❌ Passwords do not match! Please re-type.".into(), true));
                                } else {
                                    match create_vault(&self.master_password) {
                                        Ok((meta, key)) => {
                                            self.storage.set_vault_key(Some(key));
                                            self.storage.set_vault_configured(true);
                                            let mut new_cfg = self.storage.load_config();
                                            new_cfg.vault = Some(meta);
                                            new_cfg.sessions = self.sessions.clone();
                                            new_cfg.credentials = self.credentials.clone();
                                            match self.storage.save_config(&new_cfg) {
                                                Ok(_) => {
                                                    self.master_password.clear();
                                                    self.master_password_confirm.clear();
                                                    self.vault_status = Some(("✅ Master Password enabled! All credentials in data/RustyBox.json are now encrypted with AES-256-GCM.".into(), false));
                                                }
                                                Err(e) => {
                                                    self.vault_status = Some((format!("❌ Failed to save encrypted configuration: {}", e), true));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            self.vault_status = Some((format!("❌ Key derivation error: {}", e), true));
                                        }
                                    }
                                }
                            }
                        });
                    } else if !is_unlocked {
                        // Case B: Vault is configured on disk, but currently LOCKED
                        ui.group(|ui| {
                            ui.label(RichText::new("Status: 🔒 LOCKED (AES-256-GCM Encrypted on disk)").color(Color32::from_rgb(251, 191, 36)).strong());
                            ui.label("Enter your master password to unlock credentials for session use and management.");
                            ui.add_space(8.0);

                            let mut trigger_unlock = false;
                            ui.horizontal(|ui| {
                                ui.label("Master Password:");
                                let pass_edit = ui.add(TextEdit::singleline(&mut self.master_password).password(true).desired_width(180.0));
                                if pass_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    trigger_unlock = true;
                                }
                            });

                            ui.add_space(10.0);
                            if ui.add(
                                egui::Button::new(RichText::new("🔓 Unlock Vault").strong().color(Color32::WHITE))
                                    .fill(Color32::from_rgb(56, 189, 248))
                            ).clicked()
                                || trigger_unlock
                            {
                                if let Some(ref meta) = cfg.vault {
                                    match unlock_vault(meta, &self.master_password) {
                                        Ok(key) => {
                                            self.storage.set_vault_key(Some(key));
                                            // Decrypt in memory
                                            for s in &mut self.sessions {
                                                if let Some(ref p) = s.password {
                                                    if is_encrypted(p) {
                                                        if let Ok(dec) = decrypt_str(&key, p) {
                                                            s.password = Some(dec);
                                                        }
                                                    }
                                                }
                                            }
                                            for c in &mut self.credentials {
                                                if is_encrypted(&c.password) {
                                                    if let Ok(dec) = decrypt_str(&key, &c.password) {
                                                        c.password = dec;
                                                    }
                                                }
                                            }
                                            self.master_password.clear();
                                            self.vault_status = Some(("✅ Vault successfully unlocked! Credentials decrypted in memory.".into(), false));
                                            close_vault = true;
                                        }
                                        Err(_) => {
                                            self.vault_status = Some(("❌ Incorrect master password!".into(), true));
                                        }
                                    }
                                }
                            }
                        });
                    } else {
                        // Case C: Vault is configured and currently UNLOCKED
                        ui.group(|ui| {
                            ui.label(RichText::new("Status: 🔓 UNLOCKED (Active in memory, AES-256-GCM on disk)").color(Color32::from_rgb(34, 197, 94)).strong());
                            ui.label("Credentials in data/RustyBox.json remain securely encrypted with AES-256-GCM.");
                            ui.add_space(12.0);

                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("🔒 Lock Vault Now").strong()).clicked() {
                                    self.storage.set_vault_key(None);
                                    // Reload encrypted sessions from disk into memory so memory is masked
                                    self.sessions = self.storage.load_sessions();
                                    self.credentials = self.storage.load_credentials();
                                    self.vault_status = Some(("🔒 Vault locked. Passwords masked in memory.".into(), false));
                                }

                                if ui.button(RichText::new("🔓 Remove Master Password (Store Plaintext)").color(Color32::from_rgb(239, 68, 68))).clicked() {
                                    let mut new_cfg = self.storage.load_config();
                                    new_cfg.vault = None;
                                    self.storage.set_vault_key(None);
                                    self.storage.set_vault_configured(false);
                                    new_cfg.sessions = self.sessions.clone();
                                    new_cfg.credentials = self.credentials.clone();
                                    let _ = self.storage.save_config(&new_cfg);
                                    self.vault_status = Some(("ℹ Master password removed. Passwords stored in standard format.".into(), false));
                                }
                            });
                        });
                    }

                    if let Some((ref st, is_err)) = self.vault_status {
                        ui.add_space(8.0);
                        let color = if is_err { Color32::from_rgb(239, 68, 68) } else { Color32::from_rgb(34, 197, 94) };
                        ui.label(RichText::new(st).color(color).strong());
                    }

                    ui.add_space(10.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close_vault = true;
                        }
                    });
                });
            if close_vault {
                open = false;
            }
            self.show_vault = open;
        }

        // Manage Passwords Modal (MobaXterm TFormManagePasswords: "RustyBox passwords settings")
        if self.show_manage_passwords {
            let mut open = self.show_manage_passwords;
            let mut close_dialog = false;
            egui::Window::new("RustyBox passwords settings")
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_size(Vec2::new(600.0, 480.0))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    // 1. Radio: Save sessions passwords
                    let cfg_pwd = self.storage.load_config();
                    let is_vault_configured = cfg_pwd.vault.is_some();
                    let is_vault_unlocked = self.storage.is_vault_unlocked();

                    if is_vault_configured && !is_vault_unlocked {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(38, 28, 12))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(245, 158, 11)))
                            .rounding(4.0)
                            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("🔒 Credentials Vault is LOCKED. Passwords on disk are encrypted with AES-256-GCM.").color(Color32::from_rgb(251, 191, 36)).strong());
                                    if ui.add(
                                        egui::Button::new(RichText::new("🔓 Unlock Vault...").strong().color(Color32::WHITE))
                                            .fill(Color32::from_rgb(217, 119, 6))
                                    ).clicked() {
                                        self.show_vault = true;
                                    }
                                });
                            });
                        ui.add_space(4.0);
                    }

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Save sessions passwords:").strong());
                        ui.radio_value(&mut self.manage_passwords_save_mode, 0, "Always");
                        ui.radio_value(&mut self.manage_passwords_save_mode, 1, "Never");
                        ui.radio_value(&mut self.manage_passwords_save_mode, 2, "Ask");
                    });
                    ui.checkbox(&mut self.manage_passwords_save_ssh_keys, "Save SSH keys passphrases as well");
                    ui.add_space(6.0);

                    // 2. Tabs: Passwords / Credentials
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.manage_passwords_tab == 0, "🔑 Passwords").clicked() {
                            self.manage_passwords_tab = 0;
                        }
                        if ui.selectable_label(self.manage_passwords_tab == 1, "👥 Credentials").clicked() {
                            self.manage_passwords_tab = 1;
                        }
                    });
                    ui.separator();

                    if self.manage_passwords_tab == 0 {
                        // 3. Search Filter & Show Passwords Toggle
                        ui.horizontal(|ui| {
                            ui.label("🔍 Filter:");
                            ui.add(
                                TextEdit::singleline(&mut self.manage_passwords_search)
                                    .hint_text("Search username or IP / server...")
                                    .desired_width(220.0),
                            );
                            if !self.manage_passwords_search.is_empty() && ui.small_button("✖").clicked() {
                                self.manage_passwords_search.clear();
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.checkbox(&mut self.manage_passwords_show_all, "👁 Show passwords");
                            });
                        });
                        ui.add_space(4.0);

                        // 4. Passwords Table
                        let search_term = self.manage_passwords_search.to_lowercase().trim().to_string();
                        let mut to_delete_id: Option<String> = None;

                        egui::Frame::none()
                            .fill(Color32::from_rgb(13, 17, 23))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
                            .rounding(4.0)
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                    egui::Grid::new("manage_passwords_grid")
                                        .num_columns(5)
                                        .striped(true)
                                        .spacing([14.0, 6.0])
                                        .show(ui, |ui| {
                                            ui.label(RichText::new("Protocol").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Username").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Servername").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Password").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Action").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.end_row();

                                            let mut row_count = 0;
                                            for sess in &self.sessions {
                                                let (proto, user, host) = match &sess.session_type {
                                                    SessionType::Ssh { user, host, .. } => ("SSH", user.clone(), host.clone()),
                                                    SessionType::Rdp { user, host, .. } => ("RDP", user.clone(), host.clone()),
                                                    SessionType::Ftp { user, host, .. } => ("FTP", user.clone(), host.clone()),
                                                    SessionType::Vnc { host, .. } => ("VNC", "-".into(), host.clone()),
                                                    SessionType::Telnet { host, .. } => ("TELNET", "-".into(), host.clone()),
                                                    _ => continue,
                                                };

                                                if let Some(ref pw) = sess.password {
                                                    // Search filter matches either username or host/IP
                                                    if !search_term.is_empty()
                                                        && !user.to_lowercase().contains(&search_term)
                                                        && !host.to_lowercase().contains(&search_term)
                                                    {
                                                        continue;
                                                    }
                                                    row_count += 1;

                                                    ui.label(proto);
                                                    ui.label(if user.is_empty() { "-" } else { &user });
                                                    ui.label(&host);

                                                    // Password Show/Hide
                                                    let is_pw_enc = is_encrypted(pw);
                                                    let is_unmasked = self.manage_passwords_show_all
                                                        || self.manage_passwords_unmasked_ids.contains(&sess.id);
                                                    let (pw_display, pw_col) = if is_pw_enc && !is_vault_unlocked {
                                                        ("[🔒 AES-256 Encrypted]".to_string(), Color32::from_rgb(251, 191, 36))
                                                    } else if is_unmasked {
                                                        (pw.clone(), Color32::from_rgb(226, 232, 240))
                                                    } else {
                                                        ("••••••••••••".to_string(), Color32::from_rgb(148, 163, 184))
                                                    };

                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(pw_display).family(egui::FontFamily::Monospace).color(pw_col));
                                                        if !(is_pw_enc && !is_vault_unlocked) {
                                                            let eye_icon = if is_unmasked { "🔒" } else { "👁" };
                                                            if ui.small_button(eye_icon).on_hover_text("Toggle password visibility for this row").clicked() {
                                                                if let Some(pos) = self.manage_passwords_unmasked_ids.iter().position(|id| id == &sess.id) {
                                                                    self.manage_passwords_unmasked_ids.remove(pos);
                                                                } else {
                                                                    self.manage_passwords_unmasked_ids.push(sess.id.clone());
                                                                }
                                                            }
                                                        } else {
                                                            if ui.small_button("🔓").on_hover_text("Unlock Vault to view decrypted password").clicked() {
                                                                self.show_vault = true;
                                                            }
                                                        }
                                                    });

                                                    if ui.small_button("🗑").on_hover_text("Delete saved password").clicked() {
                                                        to_delete_id = Some(sess.id.clone());
                                                    }
                                                    ui.end_row();
                                                }
                                            }

                                            if row_count == 0 {
                                                ui.label(RichText::new("No saved passwords matching filter.").weak());
                                                ui.end_row();
                                            }
                                        });
                                });
                            });

                        if let Some(del_id) = to_delete_id {
                            for s in &mut self.sessions {
                                if s.id == del_id {
                                    s.password = None;
                                }
                            }
                            let _ = self.storage.save_sessions(&self.sessions);
                        }

                        ui.add_space(4.0);
                        // Action buttons
                        ui.horizontal(|ui| {
                            if ui.button("⚡ Delete all").clicked() {
                                for s in &mut self.sessions {
                                    s.password = None;
                                }
                                let _ = self.storage.save_sessions(&self.sessions);
                            }
                            if ui.button("📁 Export to file").clicked() {
                                // Export action
                            }
                        });
                    } else {
                        // Credentials Tab (MobaXterm User Credentials Profiles)
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Shared Credentials & User Profiles").strong());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button(RichText::new("➕ New credential / user").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                                    self.new_cred_name.clear();
                                    self.new_cred_user.clear();
                                    self.new_cred_password.clear();
                                    self.show_new_cred_modal = true;
                                }
                            });
                        });
                        ui.add_space(4.0);

                        let mut to_delete_cred_idx: Option<usize> = None;
                        egui::Frame::none()
                            .fill(Color32::from_rgb(13, 17, 23))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
                            .rounding(4.0)
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                    egui::Grid::new("manage_credentials_grid")
                                        .num_columns(5)
                                        .striped(true)
                                        .spacing([14.0, 6.0])
                                        .show(ui, |ui| {
                                            ui.label(RichText::new("Profile Name").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Username").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Password").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.label(RichText::new("Action").strong().color(Color32::from_rgb(56, 189, 248)));
                                            ui.end_row();

                                            for (c_idx, cred) in self.credentials.iter().enumerate() {
                                                ui.label(&cred.name);
                                                ui.label(&cred.username);
                                                let is_cred_enc = is_encrypted(&cred.password);
                                                let cred_unmasked = self.manage_passwords_show_all
                                                    || self.manage_passwords_unmasked_ids.contains(&format!("cred_{}", c_idx));
                                                let (c_display, c_col) = if is_cred_enc && !is_vault_unlocked {
                                                    ("[🔒 AES-256 Encrypted]".to_string(), Color32::from_rgb(251, 191, 36))
                                                } else if cred_unmasked {
                                                    (cred.password.clone(), Color32::from_rgb(226, 232, 240))
                                                } else {
                                                    ("••••••••••••".to_string(), Color32::from_rgb(148, 163, 184))
                                                };
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(c_display).family(egui::FontFamily::Monospace).color(c_col));
                                                    if !(is_cred_enc && !is_vault_unlocked) {
                                                        let eye = if cred_unmasked { "🔒" } else { "👁" };
                                                        if ui.small_button(eye).on_hover_text("Toggle password visibility").clicked() {
                                                            let key = format!("cred_{}", c_idx);
                                                            if let Some(pos) = self.manage_passwords_unmasked_ids.iter().position(|id| id == &key) {
                                                                self.manage_passwords_unmasked_ids.remove(pos);
                                                            } else {
                                                                self.manage_passwords_unmasked_ids.push(key);
                                                            }
                                                        }
                                                    } else {
                                                        if ui.small_button("🔓").on_hover_text("Unlock Vault to view decrypted password").clicked() {
                                                            self.show_vault = true;
                                                        }
                                                    }
                                                });
                                                if ui.small_button("🗑").on_hover_text("Delete credential").clicked() {
                                                    to_delete_cred_idx = Some(c_idx);
                                                }
                                                ui.end_row();
                                            }

                                            if self.credentials.is_empty() {
                                                ui.label(RichText::new("No shared credentials configured. Click '+ New credential / user' above to add one.").weak());
                                                ui.end_row();
                                            }
                                        });
                                });
                            });

                        if let Some(del_idx) = to_delete_cred_idx {
                            if del_idx < self.credentials.len() {
                                self.credentials.remove(del_idx);
                                let _ = self.storage.save_credentials(&self.credentials);
                            }
                        }
                    }

                    ui.separator();
                    // 5. Storage Location & Master Password
                    ui.horizontal(|ui| {
                        ui.label("Where passwords are saved:");
                        ui.label(RichText::new("Configuration file (data/RustyBox.json)").strong().color(Color32::from_rgb(100, 220, 100)));
                        let (vault_txt, vault_col) = if !is_vault_configured {
                            ("🔒 Set Master Password...", Color32::from_rgb(56, 189, 248))
                        } else if is_vault_unlocked {
                            ("🔓 Master Password (Unlocked)", Color32::from_rgb(34, 197, 94))
                        } else {
                            ("🔒 Master Password (Locked)", Color32::from_rgb(251, 191, 36))
                        };
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(RichText::new(vault_txt).color(vault_col).strong()).clicked() {
                                self.show_vault = true;
                            }
                        });
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("✖ Cancel").clicked() {
                                close_dialog = true;
                            }
                            if ui.button("✔ OK").clicked() {
                                let _ = self.storage.save_sessions(&self.sessions);
                                let _ = self.storage.save_credentials(&self.credentials);
                                close_dialog = true;
                            }
                        });
                    });
                });
            if close_dialog {
                open = false;
            }
            self.show_manage_passwords = open;
        }

        // Add New Credential / User Profile Modal
        if self.show_new_cred_modal {
            let mut open = self.show_new_cred_modal;
            egui::Window::new("➕ New User Credential Profile")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(380.0, 240.0))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new("Enter user credentials details:").strong());
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Profile name:");
                            ui.add(TextEdit::singleline(&mut self.new_cred_name).desired_width(180.0).hint_text("e.g. Root VPS"));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Username:");
                            ui.add(TextEdit::singleline(&mut self.new_cred_user).desired_width(180.0).hint_text("e.g. root / admin"));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Password:");
                            ui.add(TextEdit::singleline(&mut self.new_cred_password).password(true).desired_width(180.0));
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("✔ Save Credential").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if !self.new_cred_user.trim().is_empty() {
                                let name = if self.new_cred_name.trim().is_empty() {
                                    self.new_cred_user.trim().to_string()
                                } else {
                                    self.new_cred_name.trim().to_string()
                                };
                                self.credentials.push(Credential {
                                    name,
                                    username: self.new_cred_user.trim().to_string(),
                                    password: self.new_cred_password.clone(),
                                });
                                let _ = self.storage.save_credentials(&self.credentials);
                                self.show_new_cred_modal = false;
                            }
                        }
                        if ui.button("✖ Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.show_new_cred_modal = false;
                        }
                    });
                });
            self.show_new_cred_modal = open;
        }

        // Global Settings & Preferences Modal (MobaXterm TFORMPARAMS)
        if self.show_settings_dialog {
            let mut open = self.show_settings_dialog;
            let mut close_dialog = false;
            egui::Window::new("⚙ RustyBox Configuration & Preferences")
                .open(&mut open)
                .default_size(Vec2::new(640.0, 480.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings_tab, 0, "📁 General");
                        ui.selectable_value(&mut self.settings_tab, 1, "💻 Terminal");
                        ui.selectable_value(&mut self.settings_tab, 2, "🖥 X11");
                        ui.selectable_value(&mut self.settings_tab, 3, "🔑 SSH");
                        ui.selectable_value(&mut self.settings_tab, 4, "🖥 RDP");
                        ui.selectable_value(&mut self.settings_tab, 5, "🎨 Display");
                        ui.selectable_value(&mut self.settings_tab, 6, "⚙ Misc");
                    });
                    ui.separator();

                    match self.settings_tab {
                        0 => {
                            // General Tab
                            ui.label(RichText::new("General Directories & Storage Settings").strong());
                            ui.add_space(4.0);
                            ui.label("Persistent Home directory: ./data/home (Relative workspace)");
                            ui.label("Configuration file: data/RustyBox.json");
                            ui.label("Automatic backup rule: data/RustyBox.json.<unixtime> (Keep 3 newest)");
                            ui.add_space(6.0);
                            ui.separator();
                            ui.label(RichText::new("Backup, Export, or Import RustyBox Sessions & Configuration").strong());
                            ui.horizontal(|ui| {
                                if ui.button("📤 Export Config (RustyBox.json)...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                                        let mut cfg = self.storage.load_config();
                                        cfg.sessions = self.sessions.clone();
                                        let _ = self.storage.export_to_file(&path, &cfg);
                                    }
                                }
                                if ui.button("📥 Import Config (RustyBox.json)...").clicked() {
                                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                                        if let Ok(imported) = self.storage.import_from_file(&path) {
                                            self.sessions = imported.sessions;
                                            let _ = self.storage.save_sessions(&self.sessions);
                                        }
                                    }
                                }
                            });
                        }
                        1 => {
                            // Terminal Tab
                            ui.label(RichText::new("Default Terminal Settings").strong());
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Default Terminal Font Size:");
                                ui.add(egui::DragValue::new(&mut self.font_size).range(9.0..=28.0).speed(0.5));
                                ui.label("px");
                            });
                            ui.horizontal(|ui| {
                                ui.label("Terminal Color Scheme:");
                                for t in crate::terminal::vt_screen::TerminalTheme::all() {
                                    ui.selectable_value(&mut self.theme, *t, t.name());
                                }
                            });
                            ui.add_space(6.0);
                            ui.checkbox(&mut self.paste_protection_enabled, "Display confirmation dialog when pasting multiple lines");
                        }
                        2 => {
                            // X11 Tab
                            ui.label(RichText::new("Embedded X11 Server Configuration").strong());
                            ui.add_space(4.0);
                            ui.label("• Automatically start X server at RustyBox startup");
                            ui.label("• Hardware-accelerated OpenGL rendering (WGL)");
                            ui.label("• Multiwindow mode (transparent desktop integration)");
                        }
                        3 => {
                            // SSH Tab
                            ui.label(RichText::new("Global SSH Engine & Agent Settings").strong());
                            ui.add_space(4.0);
                            ui.label("• Send SSH keepalive packets every 60 seconds (prevent connection drops)");
                            ui.label("• Use external Pageant / Windows OpenSSH Agent");
                            ui.label("• Use internal RustyBox Agent SSH key agent at startup");
                        }
                        4 => {
                            // RDP Tab
                            ui.label(RichText::new("Global Remote Desktop (RDP) Settings").strong());
                            ui.add_space(4.0);
                            ui.label("• Redirect local clipboard to remote Windows sessions");
                            ui.label("• Auto-reconnect on transient network drop");
                        }
                        5 => {
                            // Display Tab
                            ui.label(RichText::new("Interface & Display Preferences").strong());
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("UI Font & Interface Zoom:");
                                let mut pct = (self.ui_scale * 100.0).round() as i32;
                                if ui.add(egui::Slider::new(&mut pct, 70..=200).suffix("%").step_by(5.0)).changed() {
                                    self.ui_scale = pct as f32 / 100.0;
                                }
                                if ui.button("↺ Reset 100%").clicked() {
                                    self.ui_scale = 1.0;
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Preset UI Scales:");
                                for s in [0.8, 0.9, 1.0, 1.1, 1.25, 1.5] {
                                    let label = format!("{:.0}%", s * 100.0);
                                    if ui.selectable_label((self.ui_scale - s).abs() < 0.02, label).clicked() {
                                        self.ui_scale = s;
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            ui.label("• Display left sidebar at startup");
                            ui.label("• Use compact tab bar height");
                            ui.label("• Ask for confirmation when closing window with active sessions");
                        }
                        6 => {
                            // Misc Tab
                            ui.label(RichText::new("Miscellaneous Settings").strong());
                            ui.add_space(4.0);
                            ui.label("Integrate RustyBox into Windows Explorer context menu:");
                            ui.label("• Add 'Open RustyBox terminal here' on folders");
                            ui.label("• Add 'Run script with RustyBox shell' on .sh files");
                            ui.label("• Add 'Edit with RustyBox Quick Editor' on text/config files");
                        }
                        _ => {}
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("💾 Save Preferences").strong().color(Color32::from_rgb(56, 189, 248))).clicked() {
                            let _ = self.storage.save_sessions(&self.sessions);
                            let _ = self.storage.save_preferences(self.ui_scale, self.font_size, self.theme.name(), self.paste_protection_enabled);
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

        // Modal: Connect as...
        if let Some((target_session, mut cur_user)) = self.connect_as_target.clone() {
            let mut close_modal = false;
            let mut do_launch = false;
            egui::Window::new("👤 Connect as...")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(320.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new(format!("Session: {}", target_session.name)).strong());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut cur_user);
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("▶ Connect").strong()).clicked() {
                            do_launch = true;
                            close_modal = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });
            if do_launch {
                let mut s = target_session;
                if let SessionType::Ssh { ref mut user, .. } = s.session_type {
                    *user = cur_user.trim().to_string();
                }
                self.launch_generic_session(s);
            }
            if close_modal {
                self.connect_as_target = None;
            } else if let Some((_, ref mut u)) = self.connect_as_target {
                *u = cur_user;
            }
        }

        // Modal: Rename session
        if let Some((target_id, mut new_name)) = self.rename_session_target.clone() {
            let mut close_modal = false;
            let mut do_rename = false;
            egui::Window::new("✏️ Rename session")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(320.0)
                .show(ctx, |ui| {
                    ui.label("New session name:");
                    ui.text_edit_singleline(&mut new_name);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Rename").strong()).clicked() {
                            do_rename = true;
                            close_modal = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });
            if do_rename {
                let clean_name = new_name.trim().to_string();
                if !clean_name.is_empty() {
                    if let Some(s) = self.sessions.iter_mut().find(|s| s.id == target_id) {
                        s.name = clean_name;
                    }
                    let _ = self.storage.save_sessions(&self.sessions);
                }
            }
            if close_modal {
                self.rename_session_target = None;
            } else if let Some((_, ref mut n)) = self.rename_session_target {
                *n = new_name;
            }
        }

        // Modal: Rename folder
        if let Some((old_folder, mut new_folder)) = self.rename_folder_target.clone() {
            let mut close_modal = false;
            let mut do_rename = false;
            egui::Window::new("📁 Rename folder")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(320.0)
                .show(ctx, |ui| {
                    ui.label("New folder name:");
                    ui.text_edit_singleline(&mut new_folder);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Rename").strong()).clicked() {
                            do_rename = true;
                            close_modal = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });
            if do_rename {
                let clean_folder = new_folder.trim().to_string();
                if !clean_folder.is_empty() {
                    for s in &mut self.sessions {
                        if s.folder == old_folder {
                            s.folder = clean_folder.clone();
                        }
                    }
                    let _ = self.storage.save_sessions(&self.sessions);
                }
            }
            if close_modal {
                self.rename_folder_target = None;
            } else if let Some((_, ref mut f)) = self.rename_folder_target {
                *f = new_folder;
            }
        }

        // Modal: Rename Tab
        if let Some((tab_idx, mut new_tab_name)) = self.rename_tab_target.clone() {
            let mut close_modal = false;
            let mut do_rename = false;
            egui::Window::new("✏ Rename Tab")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .default_width(320.0)
                .show(ctx, |ui| {
                    ui.label("Enter new tab title:");
                    ui.text_edit_singleline(&mut new_tab_name);
                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("✓ Rename").strong()).clicked() {
                            do_rename = true;
                            close_modal = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });
            if do_rename {
                let clean_name = new_tab_name.trim().to_string();
                if !clean_name.is_empty() {
                    if let Some(tab) = self.open_tabs.get_mut(tab_idx) {
                        tab.title = clean_name;
                    }
                }
            }
            if close_modal {
                self.rename_tab_target = None;
            } else if let Some((_, ref mut n)) = self.rename_tab_target {
                *n = new_tab_name;
            }
        }

        // Modal: Import Sessions (MobaXterm / .txt / .ini / .mxtpro / JSON)
        self.render_import_dialog(ctx);

        // Status Toast Notification
        if let Some((msg, created_at)) = &self.status_toast {
            if created_at.elapsed().as_secs() < 3 {
                egui::Area::new(egui::Id::new("status_toast"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -40.0))
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(31, 111, 235))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(56, 189, 248)))
                            .rounding(4.0)
                            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(msg).color(Color32::WHITE).strong());
                            });
                    });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_remote_session_prompt_and_motd() {
        let screen = r#"
Welcome to Ubuntu 22.04.4 LTS (GNU/Linux 5.15.0-105-generic x86_64)

  System information as of Tue Nov  3 10:25:22 UTC 2026

  System load:                      0.12
  Usage of /:                       29.0% of 22.88GB
  Memory usage:                     33%
  Swap usage:                       1%
  Processes:                        169
  Users logged in:                  0
  IPv4 address for enp1s0:          198.51.100.42

demouser1@vultr:~$ 
"#;
        let detected = detect_remote_session("", screen, "LONG-PC").expect("should detect remote session");
        assert_eq!(detected.user, "demouser1");
        assert_eq!(detected.host, "vultr");
        assert_eq!(detected.disk_pct, Some(29));
        assert_eq!(detected.ram_pct, Some(33));
        assert_eq!(detected.cpu_pct, Some(1));
    }

    #[test]
    fn test_detect_remote_session_from_title() {
        let title = "root@198.51.100.42: ~";
        let screen = "root@198.51.100.42:~# ls\n";
        let detected = detect_remote_session(title, screen, "LONG-PC").expect("should detect remote session");
        assert_eq!(detected.user, "root");
        assert_eq!(detected.host, "198.51.100.42");
    }

    #[test]
    fn test_detect_local_windows_and_linux_prompt() {
        let win_screen = "C:\\Users\\Long> dir\n";
        assert!(detect_remote_session("", win_screen, "LONG-PC").is_none());

        let local_linux_screen = "longkaka@LONG-PC:~$ ls\n";
        assert!(detect_remote_session("", local_linux_screen, "LONG-PC").is_none());
    }
}
