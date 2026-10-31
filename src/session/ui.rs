use super::{AuthMethod, Session, SessionType};
use egui::{RichText, TextEdit, Vec2};

pub struct NewSessionDialog {
    pub is_open: bool,
    pub session_name: String,
    pub folder_name: String,
    pub protocol_tab: usize, // 0: SSH, 1: Local Shell, 2: Serial, 3: Telnet, 4: RDP, 5: VNC, 6: FTP
    // SSH
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_auth: AuthMethod,
    pub ssh_key_path: String,
    pub ssh_jump_host: String,
    pub ssh_auto_sftp: bool,
    pub ssh_x11: bool,
    // Local
    pub local_shell: String,
    pub local_workdir: String,
    // Serial
    pub serial_port: String,
    pub serial_baud: u32,
    // Telnet / Generic
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
}

impl NewSessionDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            session_name: "New Session".into(),
            folder_name: "Servers".into(),
            protocol_tab: 0,
            ssh_host: "".into(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_auth: AuthMethod::Password,
            ssh_key_path: "".into(),
            ssh_jump_host: "".into(),
            ssh_auto_sftp: true,
            ssh_x11: false,
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
        }
    }

    pub fn open_for_new(&mut self) {
        self.is_open = true;
        self.session_name = "New Session".into();
    }

    pub fn to_session(&self) -> Session {
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
            6 => SessionType::Ftp {
                host: self.ftp_host.trim().to_string(),
                port: self.ftp_port,
                user: self.ftp_user.trim().to_string(),
            },
            _ => SessionType::Ssh {
                host: self.ssh_host.trim().to_string(),
                port: self.ssh_port,
                user: self.ssh_user.trim().to_string(),
                auth_method: self.ssh_auth.clone(),
                key_path: if self.ssh_key_path.trim().is_empty() {
                    None
                } else {
                    Some(self.ssh_key_path.trim().to_string())
                },
                jump_host: if self.ssh_jump_host.trim().is_empty() {
                    None
                } else {
                    Some(self.ssh_jump_host.trim().to_string())
                },
                tunnel_x11: self.ssh_x11,
            },
        };

        Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: if self.session_name.trim().is_empty() {
                match self.protocol_tab {
                    0 => self.ssh_host.clone(),
                    1 => "Local Shell".into(),
                    2 => self.serial_port.clone(),
                    3 => self.telnet_host.clone(),
                    4 => format!("RDP: {}", self.rdp_host),
                    5 => format!("VNC: {}", self.vnc_host),
                    6 => format!("FTP: {}", self.ftp_host),
                    _ => "Session".to_string(),
                }
            } else {
                self.session_name.clone()
            },
            folder: if self.folder_name.trim().is_empty() {
                "Default".to_string()
            } else {
                self.folder_name.clone()
            },
            session_type,
            auto_sftp: self.ssh_auto_sftp,
            created_at: chrono::Local::now().to_rfc3339(),
        }
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

    egui::Window::new("⚙ New Session Settings (MobaXterm Style)")
        .open(&mut open)
        .resizable(false)
        .default_size(Vec2::new(540.0, 460.0))
        .show(ctx, |ui| {
            // Protocol Tabs
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut dialog.protocol_tab, 0, "🌐 SSH");
                ui.selectable_value(&mut dialog.protocol_tab, 1, "💻 Local Shell");
                ui.selectable_value(&mut dialog.protocol_tab, 2, "🔌 Serial / COM");
                ui.selectable_value(&mut dialog.protocol_tab, 3, "📡 Telnet");
                ui.selectable_value(&mut dialog.protocol_tab, 4, "🖥 RDP");
                ui.selectable_value(&mut dialog.protocol_tab, 5, "👁 VNC");
                ui.selectable_value(&mut dialog.protocol_tab, 6, "📁 FTP/SFTP");
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Session Name:");
                ui.text_edit_singleline(&mut dialog.session_name);
            });
            ui.horizontal(|ui| {
                ui.label("Folder / Group:");
                ui.text_edit_singleline(&mut dialog.folder_name);
            });

            ui.separator();

            match dialog.protocol_tab {
                0 => {
                    // SSH Config
                    ui.label(RichText::new("SSH Server Credentials").strong());
                    ui.horizontal(|ui| {
                        ui.label("Remote Host:");
                        ui.text_edit_singleline(&mut dialog.ssh_host);
                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut dialog.ssh_port).range(1..=65535));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut dialog.ssh_user);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Private Key:");
                        ui.text_edit_singleline(&mut dialog.ssh_key_path);
                        if ui.button("Browse...").clicked() {
                            if let Some(file) = rfd::FileDialog::new().pick_file() {
                                dialog.ssh_key_path = file.to_string_lossy().to_string();
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("SSH Gateway (Jump host):");
                        ui.add(TextEdit::singleline(&mut dialog.ssh_jump_host).hint_text("jumpuser@jumphost:22"));
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut dialog.ssh_auto_sftp, "Auto-SFTP Sidebar on connect");
                        ui.checkbox(&mut dialog.ssh_x11, "X11 Forwarding");
                    });
                }
                1 => {
                    // Local Shell
                    ui.label(RichText::new("Local Terminal Settings").strong());
                    ui.horizontal(|ui| {
                        ui.label("Shell command:");
                        ui.add(TextEdit::singleline(&mut dialog.local_shell).hint_text("Default (bash / powershell)"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Working Directory:");
                        ui.add(TextEdit::singleline(&mut dialog.local_workdir).hint_text("Default ($HOME)"));
                    });
                }
                2 => {
                    // Serial / COM
                    ui.label(RichText::new("Serial Port Configuration").strong());
                    ui.horizontal(|ui| {
                        ui.label("Port Name:");
                        ui.add(TextEdit::singleline(&mut dialog.serial_port).hint_text("COM1 or /dev/ttyUSB0"));
                        ui.label("Baud Rate:");
                        egui::ComboBox::from_id_salt("baud_rate_combo")
                            .selected_text(format!("{}", dialog.serial_baud))
                            .show_ui(ui, |ui| {
                                for rate in &[9600, 19200, 38400, 57600, 115200] {
                                    ui.selectable_value(&mut dialog.serial_baud, *rate, format!("{}", rate));
                                }
                            });
                    });
                }
                3 => {
                    // Telnet
                    ui.label(RichText::new("Telnet Server Configuration").strong());
                    ui.horizontal(|ui| {
                        ui.label("Remote Host:");
                        ui.text_edit_singleline(&mut dialog.telnet_host);
                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut dialog.telnet_port).range(1..=65535));
                    });
                }
                4 => {
                    // RDP
                    ui.label(RichText::new("Windows Remote Desktop (RDP) Settings").strong());
                    ui.horizontal(|ui| {
                        ui.label("Computer Host:");
                        ui.text_edit_singleline(&mut dialog.rdp_host);
                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut dialog.rdp_port).range(1..=65535));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut dialog.rdp_user);
                    });
                }
                5 => {
                    // VNC
                    ui.label(RichText::new("VNC Remote Desktop Settings").strong());
                    ui.horizontal(|ui| {
                        ui.label("VNC Server Host:");
                        ui.text_edit_singleline(&mut dialog.vnc_host);
                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut dialog.vnc_port).range(1..=65535));
                    });
                }
                6 => {
                    // FTP / SFTP
                    ui.label(RichText::new("FTP / Standalone File Transfer").strong());
                    ui.horizontal(|ui| {
                        ui.label("FTP Host:");
                        ui.text_edit_singleline(&mut dialog.ftp_host);
                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut dialog.ftp_port).range(1..=65535));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut dialog.ftp_user);
                    });
                }
                _ => {}
            }

            ui.add_space(15.0);
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("✔ Connect & Save").clicked() {
                    create_session = Some(dialog.to_session());
                    dialog.is_open = false;
                }
                if ui.button("Cancel").clicked() {
                    dialog.is_open = false;
                }
            });
        });

    dialog.is_open = open;
    create_session
}
