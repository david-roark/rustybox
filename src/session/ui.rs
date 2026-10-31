use super::{AuthMethod, Session, SessionType};
use egui::{RichText, TextEdit, Vec2};

pub struct NewSessionDialog {
    pub is_open: bool,
    pub session_name: String,
    pub folder_name: String,
    pub protocol_tab: usize, // 0: SSH, 1: Local Shell, 2: Serial, 3: Telnet
    // SSH
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_auth: AuthMethod,
    pub ssh_key_path: String,
    pub ssh_jump_host: String,
    pub ssh_auto_sftp: bool,
    // Local
    pub local_shell: String,
    pub local_workdir: String,
}

impl NewSessionDialog {
    pub fn new() -> Self {
        Self {
            is_open: false,
            session_name: "New SSH Session".into(),
            folder_name: "Servers".into(),
            protocol_tab: 0,
            ssh_host: "".into(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_auth: AuthMethod::Password,
            ssh_key_path: "".into(),
            ssh_jump_host: "".into(),
            ssh_auto_sftp: true,
            local_shell: "".into(),
            local_workdir: "".into(),
        }
    }

    pub fn open_for_new(&mut self) {
        self.is_open = true;
        self.session_name = "New SSH Session".into();
        self.ssh_host = "".into();
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
                tunnel_x11: false,
            },
        };

        Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: if self.session_name.trim().is_empty() {
                if !self.ssh_host.is_empty() {
                    self.ssh_host.clone()
                } else {
                    "Session".to_string()
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

    egui::Window::new("⚙ New Session Settings")
        .open(&mut open)
        .resizable(false)
        .default_size(Vec2::new(480.0, 420.0))
        .show(ctx, |ui| {
            // Protocol Tabs
            ui.horizontal(|ui| {
                ui.selectable_value(&mut dialog.protocol_tab, 0, "🌐 SSH");
                ui.selectable_value(&mut dialog.protocol_tab, 1, "💻 Local Shell");
                ui.selectable_value(&mut dialog.protocol_tab, 2, "🔌 Serial / COM");
                ui.selectable_value(&mut dialog.protocol_tab, 3, "📡 Telnet");
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
                        ui.label("Jump Host (Proxy):");
                        ui.text_edit_singleline(&mut dialog.ssh_jump_host);
                    });

                    ui.checkbox(&mut dialog.ssh_auto_sftp, "Open Auto-SFTP in sidebar on connect");
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
                _ => {
                    ui.label("Configure parameters below for this protocol:");
                    ui.horizontal(|ui| {
                        ui.label("Remote Host:");
                        ui.text_edit_singleline(&mut dialog.ssh_host);
                    });
                }
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
