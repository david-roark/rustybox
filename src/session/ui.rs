use super::{AuthMethod, Session, SessionType};
use egui::{
    pos2, vec2, Align2, Color32, FontId, Margin, Rect, RichText, Rounding, Sense, Stroke,
    TextEdit, Vec2,
};

pub struct NewSessionDialog {
    pub is_open: bool,
    pub editing_session_id: Option<String>,
    pub request_open_passwords: bool,
    pub request_open_import: bool,
    pub available_credentials: Vec<crate::session::storage::Credential>,
    pub session_name: String,
    pub folder_name: String,
    pub protocol_tab: usize, // 0: SSH, 1: Shell, 2: Serial, 3: Telnet, 4: RDP, 5: VNC, 6: FTP, 7: WSL, 8: SFTP
    pub ssh_sub_tab: usize,  // 0: Advanced SSH, 1: Terminal, 2: Network, 3: Bookmark
    pub specify_username: bool,
    pub use_private_key: bool,
    // Credentials & Password Vault
    pub password: String,
    pub save_password: bool,
    // SSH Basic & Advanced
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_auth: AuthMethod,
    pub ssh_key_path: String,
    pub ssh_jump_host: String,
    pub ssh_auto_sftp: bool,
    pub ssh_x11: bool,
    pub ssh_browser_type: String,
    pub remote_command: String,
    pub do_not_exit_after_cmd: bool,
    pub ssh_compression: bool,
    // Terminal settings tab
    pub terminal_font_size: u16,
    pub terminal_font_family: String,
    pub terminal_color_scheme: String,
    pub backspace_sends_ctrl_h: bool,
    pub right_click_paste: bool,
    pub copy_on_select: bool,
    pub log_terminal_output: bool,
    pub log_directory: String,
    // Network settings tab
    pub use_ssh_gateway: bool,
    pub gateway_host: String,
    pub gateway_port: u16,
    pub gateway_user: String,
    pub gateway_key: String,
    pub ssh_keepalive: bool,
    // Bookmark settings tab
    pub tab_color: String,
    pub comments: String,
    // Local
    pub local_shell: String,
    pub local_workdir: String,
    // Serial
    pub serial_port: String,
    pub serial_baud: u32,
    // Telnet
    pub telnet_host: String,
    pub telnet_port: u16,
    // RDP
    pub rdp_host: String,
    pub rdp_port: u16,
    pub rdp_user: String,
    // VNC
    pub vnc_host: String,
    pub vnc_port: u16,
    // FTP
    pub ftp_host: String,
    pub ftp_port: u16,
    pub ftp_user: String,
    // WSL
    pub wsl_distro: String,
    pub wsl_user: String,
    pub wsl_workdir: String,
    // Sub-dialogs inside Session settings (RustyBox DFM child popups)
    pub show_expert_ssh: bool,
    pub show_expert_rdp: bool,
    pub show_expert_term: bool,
    pub show_jump_hosts: bool,
    pub show_color_selector: bool,
    pub show_font_selector: bool,
    pub show_syntax_customizer: bool,
    pub ssh_proto_version: String,
    pub ssh_cipher_order: String,
    pub ssh_kex_algo: String,
    pub ssh_hostkey_algo: String,
    pub ssh_agent_forwarding: bool,
    pub ssh_gssapi_auth: bool,
    pub ssh_strict_hostkey: bool,
    pub rdp_resolution: String,
    pub rdp_color_depth: String,
    pub rdp_redirect_clipboard: bool,
    pub rdp_redirect_audio: bool,
    pub rdp_redirect_drives: bool,
    pub rdp_nla_security: bool,
    pub term_type_str: String,
    pub term_charset: String,
    pub term_scrollback_lines: u32,
    pub term_bell_mode: String,
    pub term_cursor_shape: usize,
    pub term_cursor_blink: bool,
    pub startup_macro: String,
    pub validation_error: Option<String>,
}

impl NewSessionDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            editing_session_id: None,
            request_open_passwords: false,
            request_open_import: false,
            available_credentials: Vec::new(),
            session_name: "".into(),
            folder_name: "Servers".into(),
            protocol_tab: 0,
            ssh_sub_tab: 0,
            specify_username: true,
            use_private_key: false,
            password: "".into(),
            save_password: true,
            ssh_host: "".into(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_auth: AuthMethod::Password,
            ssh_key_path: "".into(),
            ssh_jump_host: "".into(),
            ssh_auto_sftp: true,
            ssh_x11: true,
            ssh_browser_type: "SFTP protocol".into(),
            remote_command: "".into(),
            do_not_exit_after_cmd: false,
            ssh_compression: true,
            terminal_font_size: 14,
            terminal_font_family: String::new(),
            terminal_color_scheme: "RustyBox Dark (Default)".into(),
            backspace_sends_ctrl_h: false,
            right_click_paste: true,
            copy_on_select: true,
            log_terminal_output: false,
            log_directory: "".into(),
            use_ssh_gateway: false,
            gateway_host: "".into(),
            gateway_port: 22,
            gateway_user: "root".into(),
            gateway_key: "".into(),
            ssh_keepalive: true,
            tab_color: "Default (Blue)".into(),
            comments: "".into(),
            local_shell: "".into(),
            local_workdir: "".into(),
            serial_port: "COM1".into(),
            serial_baud: 115200,
            telnet_host: "".into(),
            telnet_port: 23,
            rdp_host: "".into(),
            rdp_port: 3389,
            rdp_user: "Administrator".into(),
            vnc_host: "".into(),
            vnc_port: 5900,
            ftp_host: "".into(),
            ftp_port: 21,
            ftp_user: "anonymous".into(),
            wsl_distro: "Ubuntu".into(),
            wsl_user: "".into(),
            wsl_workdir: "".into(),
            show_expert_ssh: false,
            show_expert_rdp: false,
            show_expert_term: false,
            show_jump_hosts: false,
            show_color_selector: false,
            show_font_selector: false,
            show_syntax_customizer: false,
            ssh_proto_version: "SSHv2 only (Recommended)".into(),
            ssh_cipher_order: "chacha20-poly1305, aes256-gcm, aes128-ctr".into(),
            ssh_kex_algo: "curve25519-sha256, diffie-hellman-group-exchange-sha256".into(),
            ssh_hostkey_algo: "ssh-ed25519, rsa-sha2-512, ecdsa-sha2-nistp256".into(),
            ssh_agent_forwarding: false,
            ssh_gssapi_auth: false,
            ssh_strict_hostkey: false,
            rdp_resolution: "Fit to window (Dynamic)".into(),
            rdp_color_depth: "32-bit TrueColor".into(),
            rdp_redirect_clipboard: true,
            rdp_redirect_audio: true,
            rdp_redirect_drives: false,
            rdp_nla_security: true,
            term_type_str: "xterm-256color".into(),
            term_charset: "UTF-8 (Unicode)".into(),
            term_scrollback_lines: 10000,
            term_bell_mode: "Visual bell (flash)".into(),
            term_cursor_shape: 0,
            term_cursor_blink: true,
            startup_macro: "None".into(),
            validation_error: None,
        }
    }

    pub fn open_for_new(&mut self) {
        self.is_open = true;
        self.validation_error = None;
        self.editing_session_id = None;
        self.protocol_tab = 0; // Default to SSH protocol tab
        self.session_name.clear();
        self.password.clear();
        self.save_password = true;
        self.specify_username = true;
        self.use_private_key = false;
        self.ssh_sub_tab = 0;
        self.remote_command.clear();
        self.comments.clear();
    }

    pub fn open_for_new_in_folder(&mut self, folder: &str) {
        self.open_for_new();
        self.folder_name = folder.to_string();
    }

    pub fn open_for_edit(&mut self, s: &Session) {
        self.is_open = true;
        self.validation_error = None;
        self.editing_session_id = Some(s.id.clone());
        self.session_name = s.name.clone();
        self.folder_name = s.folder.clone();
        self.password = s.password.clone().unwrap_or_default();
        self.save_password = s.save_password;
        self.ssh_auto_sftp = s.auto_sftp;
        self.ssh_sub_tab = 0;

        self.ssh_browser_type = if s.ssh_browser_type.is_empty() {
            "SFTP protocol".into()
        } else {
            s.ssh_browser_type.clone()
        };
        self.remote_command = s.remote_command.clone();
        self.do_not_exit_after_cmd = s.do_not_exit_after_cmd;
        self.ssh_compression = s.ssh_compression;
        self.terminal_font_size = if s.terminal_font_size == 0 {
            14
        } else {
            s.terminal_font_size
        };
        self.terminal_font_family = s.terminal_font_family.clone();
        self.terminal_color_scheme = if s.terminal_color_scheme.is_empty() {
            "RustyBox Dark (Default)".into()
        } else {
            s.terminal_color_scheme.clone()
        };
        self.backspace_sends_ctrl_h = s.backspace_sends_ctrl_h;
        self.right_click_paste = s.right_click_paste;
        self.copy_on_select = s.copy_on_select;
        self.log_terminal_output = s.log_terminal_output;
        self.log_directory = s.log_directory.clone();
        self.use_ssh_gateway = s.use_ssh_gateway;
        self.gateway_host = s.gateway_host.clone();
        self.gateway_port = if s.gateway_port == 0 { 22 } else { s.gateway_port };
        self.gateway_user = s.gateway_user.clone();
        self.gateway_key = s.gateway_key.clone();
        self.ssh_keepalive = s.ssh_keepalive;
        self.tab_color = if s.tab_color.is_empty() {
            "Default (Blue)".into()
        } else {
            s.tab_color.clone()
        };
        self.comments = s.comments.clone();

        match &s.session_type {
            SessionType::Ssh {
                host,
                port,
                user,
                auth_method,
                key_path,
                jump_host,
                tunnel_x11,
            } => {
                self.protocol_tab = 0;
                self.ssh_host = host.clone();
                self.ssh_port = *port;
                self.ssh_user = user.clone();
                self.specify_username = !user.trim().is_empty();
                self.ssh_auth = auth_method.clone();
                self.ssh_key_path = key_path.clone().unwrap_or_default();
                self.use_private_key = !self.ssh_key_path.trim().is_empty();
                self.ssh_jump_host = jump_host.clone().unwrap_or_default();
                if !self.ssh_jump_host.is_empty() && self.gateway_host.is_empty() {
                    self.use_ssh_gateway = true;
                    self.gateway_host = self.ssh_jump_host.clone();
                }
                self.ssh_x11 = *tunnel_x11;
            }
            SessionType::LocalShell {
                custom_shell,
                working_dir,
            } => {
                self.protocol_tab = 1;
                self.local_shell = custom_shell.clone().unwrap_or_default();
                self.local_workdir = working_dir.clone().unwrap_or_default();
            }
            SessionType::Serial {
                port_name,
                baud_rate,
            } => {
                self.protocol_tab = 2;
                self.serial_port = port_name.clone();
                self.serial_baud = *baud_rate;
            }
            SessionType::Telnet { host, port } => {
                self.protocol_tab = 3;
                self.telnet_host = host.clone();
                self.telnet_port = *port;
            }
            SessionType::Rdp { host, port, user } => {
                self.protocol_tab = 4;
                self.rdp_host = host.clone();
                self.rdp_port = *port;
                self.rdp_user = user.clone();
            }
            SessionType::Vnc { host, port } => {
                self.protocol_tab = 5;
                self.vnc_host = host.clone();
                self.vnc_port = *port;
            }
            SessionType::Ftp { host, port, user } => {
                self.protocol_tab = 6;
                self.ftp_host = host.clone();
                self.ftp_port = *port;
                self.ftp_user = user.clone();
            }
            SessionType::Wsl {
                distro,
                user,
                working_dir,
            } => {
                self.protocol_tab = 7;
                self.wsl_distro = distro.clone();
                self.wsl_user = user.clone().unwrap_or_default();
                self.wsl_workdir = working_dir.clone().unwrap_or_default();
            }
        }
    }

    pub fn to_session(&self) -> Session {
        let effective_user = if self.specify_username && !self.ssh_user.trim().is_empty() {
            self.ssh_user.trim().to_string()
        } else {
            "root".to_string()
        };

        let jump_str = if self.use_ssh_gateway && !self.gateway_host.trim().is_empty() {
            if self.gateway_user.trim().is_empty() {
                Some(format!("{}:{}", self.gateway_host.trim(), self.gateway_port))
            } else {
                Some(format!(
                    "{}@{}:{}",
                    self.gateway_user.trim(),
                    self.gateway_host.trim(),
                    self.gateway_port
                ))
            }
        } else if !self.ssh_jump_host.trim().is_empty() {
            Some(self.ssh_jump_host.trim().to_string())
        } else {
            None
        };

        let session_type = match self.protocol_tab {
            1 => SessionType::LocalShell {
                custom_shell: if self.local_shell.trim().is_empty() {
                    None
                } else {
                    Some(self.local_shell.trim().to_string())
                },
                working_dir: if self.local_workdir.trim().is_empty() {
                    None
                } else {
                    Some(self.local_workdir.trim().to_string())
                },
            },
            2 => SessionType::Serial {
                port_name: self.serial_port.trim().to_string(),
                baud_rate: self.serial_baud,
            },
            3 => SessionType::Telnet {
                host: self.telnet_host.trim().to_string(),
                port: self.telnet_port,
            },
            4 => SessionType::Rdp {
                host: self.rdp_host.trim().to_string(),
                port: self.rdp_port,
                user: self.rdp_user.trim().to_string(),
            },
            5 => SessionType::Vnc {
                host: self.vnc_host.trim().to_string(),
                port: self.vnc_port,
            },
            6 | 8 => SessionType::Ftp {
                host: self.ftp_host.trim().to_string(),
                port: self.ftp_port,
                user: self.ftp_user.trim().to_string(),
            },
            7 => SessionType::Wsl {
                distro: if self.wsl_distro.trim().is_empty() {
                    "Ubuntu".into()
                } else {
                    self.wsl_distro.trim().to_string()
                },
                user: if self.wsl_user.trim().is_empty() {
                    None
                } else {
                    Some(self.wsl_user.trim().to_string())
                },
                working_dir: if self.wsl_workdir.trim().is_empty() {
                    None
                } else {
                    Some(self.wsl_workdir.trim().to_string())
                },
            },
            _ => SessionType::Ssh {
                host: self.ssh_host.trim().to_string(),
                port: self.ssh_port,
                user: effective_user,
                auth_method: if self.use_private_key && !self.ssh_key_path.trim().is_empty() {
                    AuthMethod::PrivateKey
                } else {
                    self.ssh_auth.clone()
                },
                key_path: if self.use_private_key && !self.ssh_key_path.trim().is_empty() {
                    Some(self.ssh_key_path.trim().to_string())
                } else {
                    None
                },
                jump_host: jump_str,
                tunnel_x11: self.ssh_x11,
            },
        };

        let id = self
            .editing_session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        Session {
            id,
            name: if self.session_name.trim().is_empty() {
                match self.protocol_tab {
                    0 => {
                        if self.ssh_host.trim().is_empty() {
                            "New SSH".into()
                        } else {
                            self.ssh_host.trim().to_string()
                        }
                    }
                    1 => "Local Shell".into(),
                    2 => self.serial_port.clone(),
                    3 => self.telnet_host.clone(),
                    4 => format!("RDP: {}", self.rdp_host),
                    5 => format!("VNC: {}", self.vnc_host),
                    6 => format!("FTP: {}", self.ftp_host),
                    8 => format!("SFTP: {}", self.ftp_host),
                    7 => format!("WSL: {}", self.wsl_distro),
                    _ => "Session".to_string(),
                }
            } else {
                self.session_name.trim().to_string()
            },
            folder: if self.folder_name.trim().is_empty() {
                "Default".to_string()
            } else {
                self.folder_name.trim().to_string()
            },
            session_type,
            auto_sftp: self.ssh_auto_sftp,
            created_at: chrono::Local::now().to_rfc3339(),
            password: if !self.password.trim().is_empty() {
                Some(self.password.clone())
            } else {
                None
            },
            save_password: self.save_password,
            ssh_browser_type: self.ssh_browser_type.clone(),
            remote_command: self.remote_command.clone(),
            do_not_exit_after_cmd: self.do_not_exit_after_cmd,
            ssh_compression: self.ssh_compression,
            terminal_font_size: self.terminal_font_size,
            terminal_font_family: self.terminal_font_family.clone(),
            terminal_color_scheme: self.terminal_color_scheme.clone(),
            backspace_sends_ctrl_h: self.backspace_sends_ctrl_h,
            right_click_paste: self.right_click_paste,
            copy_on_select: self.copy_on_select,
            log_terminal_output: self.log_terminal_output,
            log_directory: self.log_directory.clone(),
            use_ssh_gateway: self.use_ssh_gateway,
            gateway_host: self.gateway_host.clone(),
            gateway_port: self.gateway_port,
            gateway_user: self.gateway_user.clone(),
            gateway_key: self.gateway_key.clone(),
            ssh_keepalive: self.ssh_keepalive,
            tab_color: self.tab_color.clone(),
            comments: self.comments.clone(),
            last_connected_at: None,
            connect_count: 0,
        }
    }
}

/// Crisp RustyBox square checkbox with vector checkmark and clean spacing (never circular, never overlaps label)
pub fn rusty_checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
    let font_id = FontId::proportional(12.5);
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        font_id.clone(),
        Color32::from_rgb(230, 237, 243),
    );
    let box_sz = 16.0;
    let gap = 8.0;
    let total_w = box_sz + gap + galley.size().x + 2.0;
    let total_h = galley.size().y.max(box_sz + 4.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(total_w, total_h), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        *checked = !*checked;
    }

    let box_rect = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - box_sz * 0.5),
        vec2(box_sz, box_sz),
    );
    let p = ui.painter();
    let bg = if *checked {
        Color32::from_rgb(31, 111, 235)
    } else if resp.hovered() {
        Color32::from_rgb(33, 38, 45)
    } else {
        Color32::from_rgb(13, 17, 23)
    };
    let border = if *checked || resp.hovered() {
        Color32::from_rgb(56, 189, 248)
    } else {
        Color32::from_rgb(72, 79, 88)
    };
    p.rect(box_rect, Rounding::same(3.0), bg, Stroke::new(1.2, border));

    if *checked {
        let c = box_rect.center();
        let ck = Stroke::new(1.8, Color32::WHITE);
        p.line_segment([pos2(c.x - 3.5, c.y + 0.2), pos2(c.x - 1.0, c.y + 2.8)], ck);
        p.line_segment([pos2(c.x - 1.0, c.y + 2.8), pos2(c.x + 3.8, c.y - 2.6)], ck);
    }

    p.galley(
        pos2(box_rect.right() + gap, rect.center().y - galley.size().y * 0.5),
        galley,
        Color32::from_rgb(230, 237, 243),
    );
    resp
}

/// Centered, balanced RustyBox OK / Cancel footer button pair
pub fn rusty_ok_cancel_buttons(
    ui: &mut egui::Ui,
    ok_label: &str,
    cancel_label: &str,
) -> (bool, bool) {
    let mut ok_clicked = false;
    let mut cancel_clicked = false;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let btn_w = 115.0;
        let btn_h = 32.0;
        let spacing = 16.0;
        let total_w = btn_w * 2.0 + spacing;
        let pad = ((ui.available_width() - total_w) * 0.5).max(0.0);
        ui.add_space(pad);

        let ok_btn = egui::Button::new(
            RichText::new(ok_label)
                .size(13.0)
                .strong()
                .color(Color32::WHITE),
        )
        .min_size(vec2(btn_w, btn_h))
        .fill(Color32::from_rgb(35, 134, 54))
        .stroke(Stroke::new(1.0, Color32::from_rgb(63, 185, 80)));

        if ui.add(ok_btn).clicked() {
            ok_clicked = true;
        }

        ui.add_space(spacing);

        let cancel_btn = egui::Button::new(
            RichText::new(cancel_label)
                .size(13.0)
                .strong()
                .color(Color32::from_rgb(240, 246, 252)),
        )
        .min_size(vec2(btn_w, btn_h))
        .fill(Color32::from_rgb(55, 65, 81))
        .stroke(Stroke::new(1.0, Color32::from_rgb(107, 114, 128)));

        if ui.add(cancel_btn).clicked() {
            cancel_clicked = true;
        }
    });
    (ok_clicked, cancel_clicked)
}

/// Centered single primary button for RustyBox dialogs with only one close/OK button
pub fn rusty_close_button(ui: &mut egui::Ui, label: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let btn_w = 104.0;
        let pad = ((ui.available_width() - btn_w) * 0.5).max(0.0);
        ui.add_space(pad);
        let btn = egui::Button::new(
            RichText::new(label)
                .size(12.5)
                .strong()
                .color(Color32::WHITE),
        )
        .min_size(vec2(btn_w, 29.0))
        .fill(Color32::from_rgb(31, 111, 235))
        .stroke(Stroke::new(1.0, Color32::from_rgb(56, 189, 248)));
        if ui.add(btn).clicked() {
            clicked = true;
        }
    });
    clicked
}

/// Draws a crisp vector icon for each of the 9 protocols in the top RustyBox protocol selector bar
fn paint_protocol_icon(p: &egui::Painter, center: egui::Pos2, label: &str, accent: Color32) {
    let badge = Rect::from_center_size(center, vec2(26.0, 18.0));
    p.rect(
        badge,
        Rounding::same(4.0),
        Color32::from_rgb(13, 17, 23),
        Stroke::new(1.1, accent),
    );
    let s = Stroke::new(1.5, accent);
    let c = center;

    match label {
        "SSH" => {
            // Golden key
            p.circle_stroke(pos2(c.x - 3.8, c.y), 2.8, s);
            p.line_segment([pos2(c.x - 1.0, c.y), pos2(c.x + 6.0, c.y)], s);
            p.line_segment([pos2(c.x + 3.2, c.y), pos2(c.x + 3.2, c.y + 2.6)], s);
            p.line_segment([pos2(c.x + 5.5, c.y), pos2(c.x + 5.5, c.y + 2.6)], s);
        }
        "Telnet" => {
            // Terminal screen with >_
            p.line_segment([pos2(c.x - 6.0, c.y - 3.0), pos2(c.x - 2.0, c.y)], s);
            p.line_segment([pos2(c.x - 6.0, c.y + 3.0), pos2(c.x - 2.0, c.y)], s);
            p.line_segment([pos2(c.x + 0.5, c.y + 3.0), pos2(c.x + 6.0, c.y + 3.0)], s);
        }
        "RDP" => {
            // Desktop monitor with stand
            let mon = Rect::from_center_size(pos2(c.x, c.y - 1.2), vec2(12.0, 8.0));
            p.rect_stroke(mon, Rounding::same(1.2), s);
            p.line_segment([pos2(c.x, mon.bottom()), pos2(c.x, mon.bottom() + 2.8)], s);
            p.line_segment(
                [pos2(c.x - 3.5, mon.bottom() + 2.8), pos2(c.x + 3.5, mon.bottom() + 2.8)],
                s,
            );
        }
        "VNC" => {
            // Dual overlapping remote screens
            let back = Rect::from_center_size(pos2(c.x - 1.8, c.y - 1.2), vec2(9.0, 6.5));
            let front = Rect::from_center_size(pos2(c.x + 1.8, c.y + 1.2), vec2(9.0, 6.5));
            p.rect_stroke(back, Rounding::same(1.0), s);
            p.rect_filled(front, Rounding::same(1.0), Color32::from_rgb(13, 17, 23));
            p.rect_stroke(front, Rounding::same(1.0), s);
        }
        "FTP" => {
            // Up/down transfer arrows
            p.line_segment([pos2(c.x - 3.5, c.y + 4.0), pos2(c.x - 3.5, c.y - 4.0)], s);
            p.line_segment([pos2(c.x - 3.5, c.y - 4.0), pos2(c.x - 6.0, c.y - 1.5)], s);
            p.line_segment([pos2(c.x - 3.5, c.y - 4.0), pos2(c.x - 1.0, c.y - 1.5)], s);

            p.line_segment([pos2(c.x + 3.5, c.y - 4.0), pos2(c.x + 3.5, c.y + 4.0)], s);
            p.line_segment([pos2(c.x + 3.5, c.y + 4.0), pos2(c.x + 1.0, c.y + 1.5)], s);
            p.line_segment([pos2(c.x + 3.5, c.y + 4.0), pos2(c.x + 6.0, c.y + 1.5)], s);
        }
        "SFTP" => {
            // Folder + lock/transfer arrow
            let fld = Rect::from_center_size(pos2(c.x, c.y + 0.8), vec2(12.0, 7.5));
            p.rect_stroke(fld, Rounding::same(1.2), s);
            p.line_segment(
                [pos2(fld.left() + 1.0, fld.top() - 1.8), pos2(fld.left() + 5.5, fld.top() - 1.8)],
                s,
            );
            p.circle_filled(pos2(c.x, c.y + 0.8), 1.6, accent);
        }
        "Serial" => {
            // DB9 Serial connector body + 5 pins
            let conn = Rect::from_center_size(c, vec2(13.0, 8.0));
            p.rect_stroke(conn, Rounding::same(2.0), s);
            for dx in [-3.5, 0.0, 3.5] {
                p.circle_filled(pos2(c.x + dx, c.y - 1.4), 1.0, accent);
            }
            for dx in [-1.8, 1.8] {
                p.circle_filled(pos2(c.x + dx, c.y + 1.6), 1.0, accent);
            }
        }
        "Shell" => {
            // Local bash prompt >_
            p.line_segment([pos2(c.x - 5.5, c.y - 3.2), pos2(c.x - 1.5, c.y)], s);
            p.line_segment([pos2(c.x - 5.5, c.y + 3.2), pos2(c.x - 1.5, c.y)], s);
            p.line_segment([pos2(c.x + 0.5, c.y + 3.2), pos2(c.x + 5.5, c.y + 3.2)], s);
        }
        "WSL" => {
            // 4-pane window grid
            let r = 3.8;
            let gap = 0.8;
            for (dx, dy) in [(-r - gap, -r + 0.5), (gap, -r + 0.5), (-r - gap, gap + 0.5), (gap, gap + 0.5)] {
                p.rect_filled(
                    Rect::from_min_size(pos2(c.x + dx, c.y + dy), vec2(3.4, 2.8)),
                    Rounding::same(0.5),
                    accent,
                );
            }
        }
        _ => {}
    }
}

pub fn render_new_session_dialog(
    ctx: &egui::Context,
    dialog: &mut NewSessionDialog,
) -> Option<Session> {
    if !dialog.is_open {
        return None;
    }

    let mut create_session = None;
    let mut open = dialog.is_open;
    let mut close_dialog = false;

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if dialog.show_expert_ssh {
            dialog.show_expert_ssh = false;
        } else if dialog.show_expert_rdp {
            dialog.show_expert_rdp = false;
        } else if dialog.show_expert_term {
            dialog.show_expert_term = false;
        } else if dialog.show_jump_hosts {
            dialog.show_jump_hosts = false;
        } else if dialog.show_color_selector {
            dialog.show_color_selector = false;
        } else if dialog.show_font_selector {
            dialog.show_font_selector = false;
        } else if dialog.show_syntax_customizer {
            dialog.show_syntax_customizer = false;
        } else {
            dialog.is_open = false;
            return None;
        }
    }

    let win_title = if dialog.editing_session_id.is_some() {
        "Session settings (Edit session)"
    } else {
        "Session settings"
    };

    egui::Window::new(win_title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size(Vec2::new(880.0, 580.0))
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(856.0);
            ui.set_max_width(856.0);

            // 1. RUSTYBOX TOP PROTOCOL SELECTOR STRIP (9 Protocols with Vector Icons)
            egui::Frame::none()
                .fill(Color32::from_rgb(15, 20, 30))
                .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let protocols: &[(&str, usize, Color32)] = &[
                        ("SSH", 0, Color32::from_rgb(251, 191, 36)),
                        ("Telnet", 3, Color32::from_rgb(165, 180, 252)),
                        ("RDP", 4, Color32::from_rgb(56, 189, 248)),
                        ("VNC", 5, Color32::from_rgb(52, 211, 153)),
                        ("FTP", 6, Color32::from_rgb(74, 222, 128)),
                        ("SFTP", 8, Color32::from_rgb(251, 146, 60)),
                        ("Serial", 2, Color32::from_rgb(244, 114, 182)),
                        ("Shell", 1, Color32::from_rgb(52, 211, 153)),
                        ("WSL", 7, Color32::from_rgb(56, 189, 248)),
                    ];

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let item_w = ((ui.available_width() - 6.0 * (protocols.len() as f32 - 1.0))
                            / protocols.len() as f32)
                            .max(62.0);

                        for &(label, tab_id, accent) in protocols {
                            let is_sel = dialog.protocol_tab == tab_id;
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(item_w, 54.0), Sense::click());
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                dialog.protocol_tab = tab_id;
                            }

                            let p = ui.painter();
                            p.rect(
                                r,
                                Rounding::same(6.0),
                                if is_sel {
                                    Color32::from_rgba_unmultiplied(37, 99, 235, 80)
                                } else if resp.hovered() {
                                    Color32::from_rgb(33, 42, 60)
                                } else {
                                    Color32::from_rgb(20, 26, 38)
                                },
                                Stroke::new(
                                    if is_sel { 1.5 } else { 1.0 },
                                    if is_sel {
                                        Color32::from_rgb(56, 189, 248)
                                    } else {
                                        Color32::from_rgb(51, 65, 85)
                                    },
                                ),
                            );

                            paint_protocol_icon(p, pos2(r.center().x, r.top() + 17.0), label, accent);

                            p.text(
                                pos2(r.center().x, r.bottom() - 11.0),
                                Align2::CENTER_CENTER,
                                label,
                                FontId::proportional(12.0),
                                if is_sel {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(226, 232, 240)
                                },
                            );
                        }
                    });
                });

            ui.add_space(8.0);

            match dialog.protocol_tab {
                0 => {
                    // 2. RUSTYBOX BASIC SSH SETTINGS BOX (Spacious, Clear & High Contrast)
                    egui::Frame::none()
                        .fill(Color32::from_rgb(18, 24, 38))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(51, 65, 85)))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                // Vector golden key icon next to "Basic SSH settings"
                                let (kr, _) = ui.allocate_exact_size(vec2(16.0, 14.0), Sense::hover());
                                let kp = ui.painter();
                                let kc = kr.center();
                                let ks = Stroke::new(1.5, Color32::from_rgb(251, 191, 36));
                                kp.circle_stroke(pos2(kc.x - 3.0, kc.y), 2.5, ks);
                                kp.line_segment([pos2(kc.x - 0.5, kc.y), pos2(kc.x + 5.5, kc.y)], ks);
                                kp.line_segment([pos2(kc.x + 3.0, kc.y), pos2(kc.x + 3.0, kc.y + 2.2)], ks);
                                kp.line_segment([pos2(kc.x + 5.0, kc.y), pos2(kc.x + 5.0, kc.y + 2.2)], ks);

                                ui.label(
                                    RichText::new("Basic SSH settings")
                                        .size(13.5)
                                        .strong()
                                        .color(Color32::from_rgb(251, 191, 36)),
                                );
                            });
                            ui.add_space(8.0);

                            // Row 1: Target host, Port & Session name
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.label(RichText::new("Remote host *").size(12.5).strong().color(Color32::from_rgb(226, 232, 240)));
                                ui.add(
                                    TextEdit::singleline(&mut dialog.ssh_host)
                                        .desired_width(260.0)
                                        .margin(Margin::symmetric(9.0, 6.0))
                                        .hint_text("e.g. 51.79.157.8 or domain.com"),
                                );
                                ui.add_space(6.0);
                                ui.label(RichText::new("Port:").size(12.5).strong().color(Color32::from_rgb(226, 232, 240)));
                                ui.add(
                                    egui::DragValue::new(&mut dialog.ssh_port)
                                        .range(1..=65535)
                                        .speed(1),
                                );
                                ui.add_space(8.0);
                                ui.label(RichText::new("Session name:").size(12.5).strong().color(Color32::from_rgb(226, 232, 240)));
                                ui.add(
                                    TextEdit::singleline(&mut dialog.session_name)
                                        .desired_width(220.0)
                                        .margin(Margin::symmetric(9.0, 6.0))
                                        .hint_text("e.g. Production Web"),
                                );
                            });

                            ui.add_space(8.0);

                            // Row 2: Folder, Username & Password
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.label(RichText::new("Folder:").size(12.5).strong().color(Color32::from_rgb(226, 232, 240)));
                                ui.add(
                                    TextEdit::singleline(&mut dialog.folder_name)
                                        .desired_width(170.0)
                                        .margin(Margin::symmetric(9.0, 6.0))
                                        .hint_text("Servers"),
                                );
                                ui.add_space(6.0);
                                rusty_checkbox(ui, &mut dialog.specify_username, "Specify username:");
                                ui.add_enabled(
                                    dialog.specify_username,
                                    TextEdit::singleline(&mut dialog.ssh_user)
                                        .desired_width(130.0)
                                        .margin(Margin::symmetric(9.0, 6.0))
                                        .hint_text("root / ubuntu"),
                                );
                                ui.add_space(6.0);
                                ui.label(RichText::new("Password:").size(12.5).strong().color(Color32::from_rgb(226, 232, 240)));
                                ui.add(
                                    TextEdit::singleline(&mut dialog.password)
                                        .password(true)
                                        .desired_width(160.0)
                                        .margin(Margin::symmetric(9.0, 6.0))
                                        .hint_text("SSH password"),
                                );
                            });

                            ui.add_space(8.0);

                            // Row 3: Password Vault, Credentials Manager, Saved Users
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                rusty_checkbox(ui, &mut dialog.save_password, "Save password in Vault");

                                ui.add_space(8.0);
                                if ui.add(egui::Button::new(RichText::new("👤🔑 Manage Credentials").size(12.0)))
                                    .on_hover_text("Manage credentials & add new user profile")
                                    .clicked()
                                {
                                    dialog.request_open_passwords = true;
                                }

                                ui.add_space(6.0);
                                egui::ComboBox::from_id_salt("ssh_cred_picker_main")
                                    .selected_text(if dialog.ssh_user.is_empty() { "Saved user profiles..." } else { &dialog.ssh_user })
                                    .width(160.0)
                                    .show_ui(ui, |ui| {
                                        if dialog.available_credentials.is_empty() {
                                            ui.label(RichText::new("No saved credentials").italics().color(Color32::GRAY));
                                        } else {
                                            for cred in &dialog.available_credentials {
                                                let label = format!("{} ({})", cred.name, cred.username);
                                                if ui.selectable_label(dialog.ssh_user == cred.username, label).clicked() {
                                                    dialog.ssh_user = cred.username.clone();
                                                    dialog.specify_username = true;
                                                    if !cred.password.is_empty() {
                                                        dialog.password = cred.password.clone();
                                                        dialog.save_password = true;
                                                    }
                                                }
                                            }
                                        }
                                    });
                            });
                        });

                    ui.add_space(8.0);

                    // 3. RUSTYBOX 4 SUB-TABS WITH VECTOR ICONS
                    let sub_tabs: &[(&str, usize, Color32)] = &[
                        ("Advanced SSH settings", 0, Color32::from_rgb(251, 191, 36)),
                        ("Terminal settings", 1, Color32::from_rgb(52, 211, 153)),
                        ("Network settings", 2, Color32::from_rgb(56, 189, 248)),
                        ("Bookmark settings", 3, Color32::from_rgb(251, 191, 36)),
                    ];
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for &(tab_label, idx, icon_col) in sub_tabs {
                            let active = dialog.ssh_sub_tab == idx;
                            let galley = ui.painter().layout_no_wrap(
                                tab_label.to_string(),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            );
                            let tab_w = galley.size().x + 30.0;
                            let (tr, tresp) =
                                ui.allocate_exact_size(vec2(tab_w, 26.0), Sense::click());
                            if tresp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if tresp.clicked() {
                                dialog.ssh_sub_tab = idx;
                            }
                            let tp = ui.painter();
                            tp.rect(
                                tr,
                                Rounding::same(5.0),
                                if active {
                                    Color32::from_rgb(31, 111, 235)
                                } else if tresp.hovered() {
                                    Color32::from_rgb(33, 38, 45)
                                } else {
                                    Color32::from_rgb(22, 27, 34)
                                },
                                Stroke::new(
                                    1.0,
                                    if active {
                                        Color32::from_rgb(56, 189, 248)
                                    } else {
                                        Color32::from_rgb(48, 54, 61)
                                    },
                                ),
                            );
                            // Mini vector icon on left of tab
                            let ic = pos2(tr.left() + 11.0, tr.center().y);
                            let is = Stroke::new(1.4, icon_col);
                            match idx {
                                0 => {
                                    tp.circle_stroke(pos2(ic.x - 2.0, ic.y), 2.2, is);
                                    tp.line_segment([pos2(ic.x, ic.y), pos2(ic.x + 4.5, ic.y)], is);
                                }
                                1 => {
                                    tp.rect_stroke(
                                        Rect::from_center_size(ic, vec2(9.0, 7.0)),
                                        Rounding::same(1.0),
                                        is,
                                    );
                                }
                                2 => {
                                    tp.circle_filled(pos2(ic.x - 2.5, ic.y + 2.0), 1.6, icon_col);
                                    tp.circle_filled(pos2(ic.x + 2.5, ic.y - 2.0), 1.6, icon_col);
                                    tp.line_segment(
                                        [pos2(ic.x - 2.5, ic.y + 2.0), pos2(ic.x + 2.5, ic.y - 2.0)],
                                        is,
                                    );
                                }
                                _ => {
                                    tp.circle_filled(ic, 2.8, icon_col);
                                }
                            }
                            tp.text(
                                pos2(tr.left() + 21.0, tr.center().y),
                                Align2::LEFT_CENTER,
                                tab_label,
                                FontId::proportional(12.0),
                                if active {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(201, 209, 217)
                                },
                            );
                        }
                    });

                    egui::Frame::none()
                        .fill(Color32::from_rgb(13, 17, 23))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.set_min_height(215.0);
                            match dialog.ssh_sub_tab {
                                0 => {
                                    // Row 1: SSH-browser type + Follow SSH path + X11-Forwarding + Compression
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 10.0;
                                        ui.label("SSH-browser type:");
                                        egui::ComboBox::from_id_salt("sftp_browser_type")
                                            .selected_text(&dialog.ssh_browser_type)
                                            .width(155.0)
                                            .show_ui(ui, |ui| {
                                                for opt in &[
                                                    "SFTP protocol",
                                                    "SCP (enhanced speed)",
                                                    "None (Disabled)",
                                                ] {
                                                    if ui
                                                        .selectable_label(
                                                            dialog.ssh_browser_type == *opt,
                                                            *opt,
                                                        )
                                                        .clicked()
                                                    {
                                                        dialog.ssh_browser_type = (*opt).to_string();
                                                        dialog.ssh_auto_sftp =
                                                            *opt != "None (Disabled)";
                                                    }
                                                }
                                            });
                                        rusty_checkbox(ui, &mut dialog.ssh_auto_sftp, "Follow SSH path");
                                        rusty_checkbox(ui, &mut dialog.ssh_x11, "X11-Forwarding");
                                        rusty_checkbox(ui, &mut dialog.ssh_compression, "Compression");
                                    });

                                    ui.add_space(10.0);

                                    // Row 2: Execute command + Do not exit after command ends
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        ui.label("Execute command:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.remote_command)
                                                .desired_width(280.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("Optional startup command (e.g. tmux attach)"),
                                        );
                                        ui.add_space(4.0);
                                        rusty_checkbox(
                                            ui,
                                            &mut dialog.do_not_exit_after_cmd,
                                            "Do not exit after command ends",
                                        );
                                    });

                                    ui.add_space(10.0);

                                    // Row 3: Use private key checkbox + path input + Browse...
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        rusty_checkbox(ui, &mut dialog.use_private_key, "Use private key:");
                                        ui.add_enabled(
                                            dialog.use_private_key,
                                            TextEdit::singleline(&mut dialog.ssh_key_path)
                                                .desired_width(370.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("~/.ssh/id_ed25519 or .pem"),
                                        );
                                        if ui
                                            .add_enabled(
                                                dialog.use_private_key,
                                                egui::Button::new("Browse..."),
                                            )
                                            .clicked()
                                        {
                                            if let Some(file) = rfd::FileDialog::new().pick_file() {
                                                dialog.ssh_key_path =
                                                    file.to_string_lossy().to_string();
                                            }
                                        }
                                    });

                                    ui.add_space(10.0);

                                    // Row 4: Advanced SSH tools
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        if ui.button("⚙ Expert SSH settings").clicked() {
                                            dialog.show_expert_ssh = true;
                                        }
                                        ui.label(
                                            RichText::new("ℹ Password and Credentials are now conveniently configured in Basic SSH settings above.")
                                                .size(11.0)
                                                .color(Color32::from_rgb(148, 163, 184)),
                                        );
                                    });
                                }
                                1 => {
                                    ui.label(
                                        RichText::new("Terminal Display & Emulation Settings")
                                            .strong()
                                            .color(Color32::from_rgb(56, 189, 248)),
                                    );
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Terminal font:");
                                        egui::ComboBox::from_id_salt("advanced_term_font_family")
                                            .selected_text(crate::terminal::fonts::terminal_font_display_name(&dialog.terminal_font_family))
                                            .width(150.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut dialog.terminal_font_family, "".to_string(), "Default (Global Setting)");
                                                for opt in crate::terminal::fonts::AVAILABLE_TERMINAL_FONTS {
                                                    ui.selectable_value(&mut dialog.terminal_font_family, opt.id.to_string(), opt.name);
                                                }
                                            });
                                        ui.add_space(8.0);
                                        ui.label("Size:");
                                        ui.add(
                                            egui::DragValue::new(&mut dialog.terminal_font_size)
                                                .range(9..=28),
                                        );
                                        if ui.button("Font settings...").clicked() {
                                            dialog.show_font_selector = true;
                                        }
                                        ui.add_space(8.0);
                                        ui.label("Color scheme:");
                                        egui::ComboBox::from_id_salt("term_color_scheme_combo")
                                            .selected_text(&dialog.terminal_color_scheme)
                                            .width(160.0)
                                            .show_ui(ui, |ui| {
                                                for scheme in &[
                                                    "RustyBox Dark (Default)",
                                                    "Dracula Pro",
                                                    "Solarized Dark",
                                                    "Solarized Light",
                                                    "Monokai Classic",
                                                    "Nord Arctic",
                                                    "Matrix Green",
                                                ] {
                                                    ui.selectable_value(
                                                        &mut dialog.terminal_color_scheme,
                                                        (*scheme).to_string(),
                                                        *scheme,
                                                    );
                                                }
                                            });
                                        if ui.button("Colors...").clicked() {
                                            dialog.show_color_selector = true;
                                        }
                                    });
                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        rusty_checkbox(
                                            ui,
                                            &mut dialog.backspace_sends_ctrl_h,
                                            "Backspace sends ^H",
                                        );
                                        ui.add_space(14.0);
                                        rusty_checkbox(
                                            ui,
                                            &mut dialog.right_click_paste,
                                            "Paste using right-click",
                                        );
                                        ui.add_space(14.0);
                                        if ui.button("Customize syntax...").clicked() {
                                            dialog.show_syntax_customizer = true;
                                        }
                                        if ui.button("Expert terminal settings...").clicked() {
                                            dialog.show_expert_term = true;
                                        }
                                    });
                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        rusty_checkbox(
                                            ui,
                                            &mut dialog.log_terminal_output,
                                            "Log terminal output to directory:",
                                        );
                                        ui.add_enabled(
                                            dialog.log_terminal_output,
                                            TextEdit::singleline(&mut dialog.log_directory)
                                                .desired_width(290.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("./logs/"),
                                        );
                                    });
                                }
                                2 => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("SSH Gateway (Jump Host / Bastion) & Keepalive")
                                                .strong()
                                                .color(Color32::from_rgb(56, 189, 248)),
                                        );
                                        ui.add_space(12.0);
                                        if ui.button("⛓ Configure multiple jump hosts...").clicked() {
                                            dialog.show_jump_hosts = true;
                                        }
                                    });
                                    ui.add_space(8.0);
                                    rusty_checkbox(
                                        ui,
                                        &mut dialog.use_ssh_gateway,
                                        "Connect through SSH gateway (jump host)",
                                    );
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Gateway host:");
                                        ui.add_enabled(
                                            dialog.use_ssh_gateway,
                                            TextEdit::singleline(&mut dialog.gateway_host)
                                                .desired_width(180.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("bastion.example.com"),
                                        );
                                        ui.label("User:");
                                        ui.add_enabled(
                                            dialog.use_ssh_gateway,
                                            TextEdit::singleline(&mut dialog.gateway_user)
                                                .desired_width(100.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("ubuntu"),
                                        );
                                        ui.label("Port:");
                                        ui.add_enabled(
                                            dialog.use_ssh_gateway,
                                            egui::DragValue::new(&mut dialog.gateway_port)
                                                .range(1..=65535),
                                        );
                                    });
                                    ui.add_space(10.0);
                                    rusty_checkbox(
                                        ui,
                                        &mut dialog.ssh_keepalive,
                                        "Send SSH keepalive packets every 60 seconds",
                                    );
                                }
                                _ => {
                                    ui.label(
                                        RichText::new("Bookmark & Folder Organization")
                                            .strong()
                                            .color(Color32::from_rgb(251, 191, 36)),
                                    );
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Session name (Bookmark):");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.session_name)
                                                .desired_width(220.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("dev-vn"),
                                        );
                                        ui.add_space(10.0);
                                        ui.label("Folder / Subfolder:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.folder_name)
                                                .desired_width(180.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("dev.track-asia.com"),
                                        );
                                    });
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Tab color:");
                                        egui::ComboBox::from_id_salt("bookmark_tab_color")
                                            .selected_text(&dialog.tab_color)
                                            .width(160.0)
                                            .show_ui(ui, |ui| {
                                                for col in &[
                                                    "Default (Blue)",
                                                    "Production (Red)",
                                                    "Staging (Amber)",
                                                    "Development (Green)",
                                                ] {
                                                    ui.selectable_value(
                                                        &mut dialog.tab_color,
                                                        (*col).to_string(),
                                                        *col,
                                                    );
                                                }
                                            });
                                        ui.add_space(10.0);
                                        ui.label("Comments:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.comments)
                                                .desired_width(240.0)
                                                .margin(Margin::symmetric(7.0, 4.0))
                                                .hint_text("Optional session notes..."),
                                        );
                                    });
                                    ui.add_space(10.0);
                                    let preview = dialog.to_session().rusty_label();
                                    ui.label(
                                        RichText::new(format!(
                                            "User sessions tree display: {}",
                                            preview
                                        ))
                                        .monospace()
                                        .color(Color32::from_rgb(56, 189, 248)),
                                    );
                                }
                            }
                        });
                }
                other_proto => {
                    let (proto_title, proto_col) = match other_proto {
                        1 => ("Basic Shell settings", Color32::from_rgb(163, 113, 247)),
                        2 => ("Basic Serial settings", Color32::from_rgb(244, 114, 182)),
                        3 => ("Basic Telnet settings", Color32::from_rgb(165, 180, 252)),
                        4 => ("Basic RDP (Remote Desktop) settings", Color32::from_rgb(56, 189, 248)),
                        5 => ("Basic VNC settings", Color32::from_rgb(52, 211, 153)),
                        6 => ("Basic FTP settings", Color32::from_rgb(251, 146, 60)),
                        8 => ("Basic SFTP settings", Color32::from_rgb(45, 212, 191)),
                        7 => ("Basic WSL (Windows Subsystem for Linux) settings", Color32::from_rgb(250, 204, 21)),
                        _ => ("Basic Session settings", Color32::from_rgb(56, 189, 248)),
                    };
                    egui::Frame::none()
                        .fill(Color32::from_rgb(22, 27, 34))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(Margin::symmetric(12.0, 9.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(RichText::new(proto_title).size(12.5).strong().color(proto_col));
                            ui.add_space(5.0);
                            match other_proto {
                                1 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Terminal shell:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.local_shell)
                                                .desired_width(240.0)
                                                .hint_text("Default (Windows: cmd / macOS+Linux: terminal shell)"),
                                        );
                                        ui.label("Startup directory:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.local_workdir)
                                                .desired_width(220.0)
                                                .hint_text("Default ($HOME)"),
                                        );
                                    });
                                }
                                2 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Serial port:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.serial_port)
                                                .desired_width(180.0)
                                                .hint_text("COM1 or /dev/ttyUSB0"),
                                        );
                                        ui.label("Speed (bps):");
                                        egui::ComboBox::from_id_salt("baud_rate_combo")
                                            .selected_text(format!("{}", dialog.serial_baud))
                                            .width(120.0)
                                            .show_ui(ui, |ui| {
                                                for rate in &[9600, 19200, 38400, 57600, 115200, 230400, 921600] {
                                                    ui.selectable_value(
                                                        &mut dialog.serial_baud,
                                                        *rate,
                                                        format!("{}", rate),
                                                    );
                                                }
                                            });
                                    });
                                }
                                3 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Remote host *");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.telnet_host)
                                                .desired_width(220.0)
                                                .hint_text("192.168.1.1"),
                                        );
                                        ui.label("Port");
                                        ui.add(egui::DragValue::new(&mut dialog.telnet_port).range(1..=65535));
                                    });
                                }
                                4 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Remote host *");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.rdp_host)
                                                .desired_width(180.0)
                                                .hint_text("10.0.0.15"),
                                        );
                                        ui.label("Username");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.rdp_user)
                                                .desired_width(130.0)
                                                .hint_text("Administrator"),
                                        );
                                        if ui.small_button("🔑").on_hover_text("Passwords & credentials settings").clicked() {
                                            dialog.request_open_passwords = true;
                                        }
                                        ui.label("Port");
                                        ui.add(egui::DragValue::new(&mut dialog.rdp_port).range(1..=65535));
                                    });
                                }
                                5 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Remote hostname or IP address *");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.vnc_host)
                                                .desired_width(220.0)
                                                .hint_text("192.168.1.50"),
                                        );
                                        ui.label("Port");
                                        ui.add(egui::DragValue::new(&mut dialog.vnc_port).range(1..=65535));
                                    });
                                }
                                6 | 8 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Remote host *");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.ftp_host)
                                                .desired_width(180.0)
                                                .hint_text("ftp.example.com"),
                                        );
                                        ui.label("Username");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.ftp_user)
                                                .desired_width(130.0)
                                                .hint_text("anonymous"),
                                        );
                                        if ui.small_button("🔑").on_hover_text("Passwords & credentials settings").clicked() {
                                            dialog.request_open_passwords = true;
                                        }
                                        ui.label("Port");
                                        ui.add(egui::DragValue::new(&mut dialog.ftp_port).range(1..=65535));
                                    });
                                }
                                _ => {
                                    ui.horizontal(|ui| {
                                        ui.label("Linux distribution:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.wsl_distro)
                                                .desired_width(180.0)
                                                .hint_text("Ubuntu"),
                                        );
                                        ui.label("User:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.wsl_user)
                                                .desired_width(120.0)
                                                .hint_text("root"),
                                        );
                                        ui.label("Startup directory:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.wsl_workdir)
                                                .desired_width(150.0)
                                                .hint_text("~"),
                                        );
                                    });
                                }
                            }
                        });

                    ui.add_space(8.0);
                    let adv_label = match other_proto {
                        1 => "Advanced Shell settings",
                        2 => "Advanced Serial settings",
                        3 => "Advanced Telnet settings",
                        4 => "Advanced RDP settings",
                        5 => "Advanced VNC settings",
                        6 => "Advanced FTP settings",
                        8 => "Advanced SFTP settings",
                        _ => "Advanced WSL settings",
                    };
                    let sub_tabs: &[(&str, usize, Color32)] = &[
                        (adv_label, 0, proto_col),
                        ("Terminal settings", 1, Color32::from_rgb(52, 211, 153)),
                        ("Network settings", 2, Color32::from_rgb(56, 189, 248)),
                        ("Bookmark settings", 3, Color32::from_rgb(251, 191, 36)),
                    ];
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for &(tab_label, idx, _) in sub_tabs {
                            let active = dialog.ssh_sub_tab == idx;
                            if ui.selectable_label(active, tab_label).clicked() {
                                dialog.ssh_sub_tab = idx;
                            }
                        }
                    });
                    egui::Frame::none()
                        .fill(Color32::from_rgb(13, 17, 23))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(48, 54, 61)))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.set_min_height(215.0);
                            match dialog.ssh_sub_tab {
                                0 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Password (Vault):");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.password)
                                                .password(true)
                                                .desired_width(210.0)
                                                .hint_text("Optional session password"),
                                        );
                                        rusty_checkbox(ui, &mut dialog.save_password, "Save in AES-256 Vault");
                                        if other_proto == 4 && ui.button("⚙ Expert RDP settings").clicked() {
                                            dialog.show_expert_rdp = true;
                                        }
                                    });
                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Execute command at startup:");
                                        ui.add(
                                            TextEdit::singleline(&mut dialog.remote_command)
                                                .desired_width(300.0)
                                                .hint_text("Optional startup command"),
                                        );
                                    });
                                    ui.add_space(10.0);
                                    ui.separator();
                                    ui.add_space(6.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Session name:");
                                        ui.add(TextEdit::singleline(&mut dialog.session_name).desired_width(210.0));
                                        ui.label("Folder / Group:");
                                        ui.add(TextEdit::singleline(&mut dialog.folder_name).desired_width(195.0));
                                    });
                                }
                                1 => {
                                    ui.horizontal(|ui| {
                                        ui.label("Terminal font:");
                                        egui::ComboBox::from_id_salt("compact_term_font_family")
                                            .selected_text(crate::terminal::fonts::terminal_font_display_name(&dialog.terminal_font_family))
                                            .width(150.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut dialog.terminal_font_family, "".to_string(), "Default (Global Setting)");
                                                for opt in crate::terminal::fonts::AVAILABLE_TERMINAL_FONTS {
                                                    ui.selectable_value(&mut dialog.terminal_font_family, opt.id.to_string(), opt.name);
                                                }
                                            });
                                        ui.add_space(8.0);
                                        ui.label("Size:");
                                        ui.add(egui::DragValue::new(&mut dialog.terminal_font_size).range(9..=28));
                                        if ui.button("Font settings...").clicked() {
                                            dialog.show_font_selector = true;
                                        }
                                        if ui.button("Colors...").clicked() {
                                            dialog.show_color_selector = true;
                                        }
                                        if ui.button("Expert terminal settings...").clicked() {
                                            dialog.show_expert_term = true;
                                        }
                                    });
                                    ui.add_space(10.0);
                                    rusty_checkbox(ui, &mut dialog.backspace_sends_ctrl_h, "Backspace sends ^H");
                                    rusty_checkbox(ui, &mut dialog.right_click_paste, "Paste using right-click");
                                    rusty_checkbox(ui, &mut dialog.copy_on_select, "Copy on select (automatically copy selected text)");
                                }
                                2 => {
                                    rusty_checkbox(ui, &mut dialog.use_ssh_gateway, "Connect through SSH gateway (jump host)");
                                    ui.add_space(8.0);
                                    if ui.button("⛓ Configure jump hosts...").clicked() {
                                        dialog.show_jump_hosts = true;
                                    }
                                }
                                _ => {
                                    ui.horizontal(|ui| {
                                        ui.label("Session name:");
                                        ui.add(TextEdit::singleline(&mut dialog.session_name).desired_width(210.0));
                                        ui.label("Folder:");
                                        ui.add(TextEdit::singleline(&mut dialog.folder_name).desired_width(180.0));
                                    });
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label("Comments:");
                                        ui.add(TextEdit::singleline(&mut dialog.comments).desired_width(340.0));
                                    });
                                }
                            }
                        });
                }
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Execute macro at session start:").size(12.0));
                egui::ComboBox::from_id_salt("session_start_macro")
                    .selected_text(&dialog.startup_macro)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for m in &["None", "htop", "docker ps", "df -h", "tmux attach"] {
                            ui.selectable_value(&mut dialog.startup_macro, (*m).to_string(), *m);
                        }
                    });
                if ui.button("↺ Reset defaults").clicked() {
                    let prev_tab = dialog.protocol_tab;
                    dialog.open_for_new();
                    dialog.protocol_tab = prev_tab;
                }
                if ui.button(RichText::new("📥 Import SSH Profiles...").color(Color32::from_rgb(56, 189, 248))).clicked() {
                    dialog.request_open_import = true;
                    close_dialog = true;
                }
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(6.0);

            if let Some(ref err) = dialog.validation_error {
                ui.add_space(2.0);
                ui.label(RichText::new(format!("⚠️ {}", err)).color(Color32::from_rgb(239, 68, 68)).strong().size(11.5));
                ui.add_space(2.0);
            }

            // 4. CENTERED RUSTYBOX OK / CANCEL BUTTONS
            let (ok_clicked, cancel_clicked) =
                rusty_ok_cancel_buttons(ui, "✔  OK", "✖  Cancel");
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ok_clicked || enter_pressed {
                let (is_valid, err_msg) = match dialog.protocol_tab {
                    0 if dialog.ssh_host.trim().is_empty() => (false, "Please specify a remote host/IP address for SSH."),
                    3 if dialog.telnet_host.trim().is_empty() => (false, "Please specify a remote host/IP address for Telnet."),
                    4 if dialog.rdp_host.trim().is_empty() => (false, "Please specify a remote host/IP address for RDP."),
                    5 if dialog.vnc_host.trim().is_empty() => (false, "Please specify a remote host/IP address for VNC."),
                    6 | 8 if dialog.ftp_host.trim().is_empty() => (false, "Please specify a remote host/IP address for FTP/SFTP."),
                    _ => (true, ""),
                };
                if is_valid {
                    dialog.validation_error = None;
                    create_session = Some(dialog.to_session());
                    close_dialog = true;
                } else {
                    dialog.validation_error = Some(err_msg.to_string());
                }
            }
            if cancel_clicked || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                dialog.validation_error = None;
                close_dialog = true;
            }
        });

    // Render RustyBox child popups of Session settings (each with working OK / Cancel / X / Esc)
    render_session_child_popups(ctx, dialog);

    dialog.is_open = open && !close_dialog;
    create_session
}

fn render_session_child_popups(ctx: &egui::Context, dialog: &mut NewSessionDialog) {
    // 1. TFormSSHProtoSettings: Advanced SSH protocol settings
    if dialog.show_expert_ssh {
        let mut open = dialog.show_expert_ssh;
        let mut close_req = false;
        egui::Window::new("Advanced SSH protocol settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("SSH protocol & algorithms").strong());
                    ui.add_space(6.0);
                    egui::Grid::new("ssh_proto_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label("Protocol version:");
                        egui::ComboBox::from_id_salt("ssh_proto_ver")
                            .selected_text(&dialog.ssh_proto_version)
                            .width(330.0)
                            .show_ui(ui, |ui| {
                                for v in &["SSHv2 only (Recommended)", "SSHv2 then SSHv1", "SSHv1 only (Legacy)"] {
                                    ui.selectable_value(&mut dialog.ssh_proto_version, (*v).to_string(), *v);
                                }
                            });
                        ui.end_row();
                        ui.label("Encryption ciphers:");
                        ui.add_sized([330.0, 24.0], TextEdit::singleline(&mut dialog.ssh_cipher_order));
                        ui.end_row();
                        ui.label("Key exchange (KEX):");
                        ui.add_sized([330.0, 24.0], TextEdit::singleline(&mut dialog.ssh_kex_algo));
                        ui.end_row();
                        ui.label("Host key algorithms:");
                        ui.add_sized([330.0, 24.0], TextEdit::singleline(&mut dialog.ssh_hostkey_algo));
                        ui.end_row();
                    });
                });
                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Authentication & security options").strong());
                    rusty_checkbox(ui, &mut dialog.ssh_agent_forwarding, "Allow SSH agent forwarding (ForwardAgent)");
                    rusty_checkbox(ui, &mut dialog.ssh_gssapi_auth, "Attempt GSSAPI / Kerberos authentication");
                    rusty_checkbox(ui, &mut dialog.ssh_strict_hostkey, "Strict host key checking (reject unknown host keys)");
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_expert_ssh = open && !close_req;
    }

    // 2. TFormAdvancedRdpSettings: Advanced RDP settings
    if dialog.show_expert_rdp {
        let mut open = dialog.show_expert_rdp;
        let mut close_req = false;
        egui::Window::new("Advanced RDP settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Display & redirection").strong());
                    ui.add_space(4.0);
                    egui::Grid::new("rdp_expert_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label("Resolution:");
                        egui::ComboBox::from_id_salt("rdp_res_combo")
                            .selected_text(&dialog.rdp_resolution)
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                for r in &["Fit to window (Dynamic)", "1920x1080 (Full HD)", "2560x1440 (QHD)", "1600x900", "1280x720"] {
                                    ui.selectable_value(&mut dialog.rdp_resolution, (*r).to_string(), *r);
                                }
                            });
                        ui.end_row();
                        ui.label("Color depth:");
                        egui::ComboBox::from_id_salt("rdp_depth_combo")
                            .selected_text(&dialog.rdp_color_depth)
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                for d in &["32-bit TrueColor", "24-bit HighColor", "16-bit Medium", "15-bit Low"] {
                                    ui.selectable_value(&mut dialog.rdp_color_depth, (*d).to_string(), *d);
                                }
                            });
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    rusty_checkbox(ui, &mut dialog.rdp_redirect_clipboard, "Enable Clipboard redirection");
                    rusty_checkbox(ui, &mut dialog.rdp_redirect_audio, "Enable Audio redirection");
                    rusty_checkbox(ui, &mut dialog.rdp_redirect_drives, "Redirect local disks to remote session");
                    rusty_checkbox(ui, &mut dialog.rdp_nla_security, "Enable Network Level Authentication (NLA / CredSSP)");
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_expert_rdp = open && !close_req;
    }

    // 3. TFormAdvancedTermSettings: Advanced terminal settings
    if dialog.show_expert_term {
        let mut open = dialog.show_expert_term;
        let mut close_req = false;
        egui::Window::new("Advanced terminal settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Terminal emulation & bell").strong());
                    ui.add_space(4.0);
                    egui::Grid::new("term_expert_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label("Terminal type ($TERM):");
                        egui::ComboBox::from_id_salt("term_type_combo")
                            .selected_text(&dialog.term_type_str)
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                for t in &["xterm-256color", "xterm", "vt100", "linux", "screen-256color"] {
                                    ui.selectable_value(&mut dialog.term_type_str, (*t).to_string(), *t);
                                }
                            });
                        ui.end_row();
                        ui.label("Remote character set:");
                        egui::ComboBox::from_id_salt("term_charset_combo")
                            .selected_text(&dialog.term_charset)
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                for c in &["UTF-8 (Unicode)", "ISO-8859-1 (Latin-1)", "CP1252 (Windows)", "EUC-JP", "GB18030"] {
                                    ui.selectable_value(&mut dialog.term_charset, (*c).to_string(), *c);
                                }
                            });
                        ui.end_row();
                        ui.label("Scrollback buffer lines:");
                        ui.add_sized([120.0, 24.0], egui::DragValue::new(&mut dialog.term_scrollback_lines).range(500..=500000).speed(100));
                        ui.end_row();
                        ui.label("Terminal bell:");
                        egui::ComboBox::from_id_salt("term_bell_combo")
                            .selected_text(&dialog.term_bell_mode)
                            .width(240.0)
                            .show_ui(ui, |ui| {
                                for b in &["Visual bell (flash)", "Audible beep", "Disabled"] {
                                    ui.selectable_value(&mut dialog.term_bell_mode, (*b).to_string(), *b);
                                }
                            });
                        ui.end_row();
                    });
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_expert_term = open && !close_req;
    }

    // 4. TFormJumpHosts: RustyBox jump hosts configuration
    if dialog.show_jump_hosts {
        let mut open = dialog.show_jump_hosts;
        let mut close_req = false;
        egui::Window::new("RustyBox jump hosts configuration")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Configure SSH gateway (bastion / jump host) before reaching target server:");
                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    rusty_checkbox(ui, &mut dialog.use_ssh_gateway, "Enable SSH gateway (jump host)");
                    ui.add_space(6.0);
                    egui::Grid::new("jump_hosts_grid").num_columns(4).spacing([10.0, 8.0]).show(ui, |ui| {
                        ui.label("Gateway host:");
                        ui.add_sized([195.0, 24.0], TextEdit::singleline(&mut dialog.gateway_host).hint_text("bastion.company.com"));
                        ui.label("Port:");
                        ui.add_sized([80.0, 24.0], egui::DragValue::new(&mut dialog.gateway_port).range(1..=65535));
                        ui.end_row();
                        ui.label("Username:");
                        ui.add_sized([195.0, 24.0], TextEdit::singleline(&mut dialog.gateway_user).hint_text("ec2-user"));
                        ui.label("Key:");
                        ui.add_sized([140.0, 24.0], TextEdit::singleline(&mut dialog.gateway_key).hint_text("Optional key"));
                        ui.end_row();
                    });
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_jump_hosts = open && !close_req;
    }

    // 5. TFormColorsSelector: Terminal colors selection
    if dialog.show_color_selector {
        let mut open = dialog.show_color_selector;
        let mut close_req = false;
        egui::Window::new("Terminal colors selection")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Select terminal color palette").strong());
                    for scheme in &[
                        "RustyBox Dark (Default)",
                        "Dracula Pro",
                        "Solarized Dark",
                        "Solarized Light",
                        "Monokai Classic",
                        "Nord Arctic",
                        "Matrix Green",
                    ] {
                        ui.radio_value(&mut dialog.terminal_color_scheme, (*scheme).to_string(), *scheme);
                    }
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_color_selector = open && !close_req;
    }

    // 6. TFormFontSelector: Terminal font selection
    if dialog.show_font_selector {
        let mut open = dialog.show_font_selector;
        let mut close_req = false;
        egui::Window::new("Terminal font selection")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Monospace terminal font").strong());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Font family:");
                        egui::ComboBox::from_id_salt("tform_font_family")
                            .selected_text(crate::terminal::fonts::terminal_font_display_name(&dialog.terminal_font_family))
                            .width(220.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut dialog.terminal_font_family, "".to_string(), "Default (Global Setting)");
                                for opt in crate::terminal::fonts::AVAILABLE_TERMINAL_FONTS {
                                    ui.selectable_value(&mut dialog.terminal_font_family, opt.id.to_string(), opt.name);
                                }
                            });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Font size (pt):");
                        ui.add(egui::DragValue::new(&mut dialog.terminal_font_size).range(9..=28));
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Cursor shape:");
                        ui.radio_value(&mut dialog.term_cursor_shape, 0, "Block");
                        ui.radio_value(&mut dialog.term_cursor_shape, 1, "Underline");
                        ui.radio_value(&mut dialog.term_cursor_shape, 2, "Vertical bar");
                    });
                    rusty_checkbox(ui, &mut dialog.term_cursor_blink, "Blinking cursor");
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_font_selector = open && !close_req;
    }

    // 7. TFormSyntax: Customize syntax highlighting
    if dialog.show_syntax_customizer {
        let mut open = dialog.show_syntax_customizer;
        let mut close_req = false;
        egui::Window::new("Customize syntax highlighting")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Active terminal keyword highlighting rules").strong());
                    ui.label("• Error / Fatal / Failed / Denied  -> Red (#F85149)");
                    ui.label("• Warning / Timeout / Deprecated -> Amber (#FBBF24)");
                    ui.label("• OK / Connected / Success / Active -> Green (#3FB950)");
                    ui.label("• IPv4 / IPv6 / MAC / URLs / Paths -> Cyan (#38BDF8)");
                });
                ui.add_space(6.0);
                let (ok, cancel) = rusty_ok_cancel_buttons(ui, "✔ OK", "✖ Cancel");
                if ok || cancel {
                    close_req = true;
                }
            });
        dialog.show_syntax_customizer = open && !close_req;
    }
}
