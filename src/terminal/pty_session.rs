use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtyPair, PtySize};
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

pub struct PtySession {
    pub parser: Arc<parking_lot::Mutex<vt100::Parser>>,
    writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    pub running: Arc<AtomicBool>,
    pub title: Arc<parking_lot::Mutex<String>>,
    pub child_pid: Option<u32>,
    pub has_new_data: Arc<AtomicBool>,
    pub child: Arc<parking_lot::Mutex<Option<Box<dyn portable_pty::Child + Send + Sync>>>>,
}

impl PtySession {
    pub fn spawn_local(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        working_dir: Option<&str>,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let mut cmd = if let Some(s) = custom_shell {
            let mut c = CommandBuilder::new(s);
            #[cfg(not(target_os = "windows"))]
            {
                c.env("TERM", "xterm-256color");
                c.env("COLORTERM", "truecolor");
            }
            c
        } else {
            #[cfg(target_os = "windows")]
            {
                let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
                CommandBuilder::new(&shell)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
                let mut c = CommandBuilder::new(&shell);
                c.env("TERM", "xterm-256color");
                c.env("COLORTERM", "truecolor");
                c
            }
        };

        if let Some(wd) = working_dir {
            cmd.cwd(wd);
        } else if let Ok(home) = std::env::var(if cfg!(target_os = "windows") { "USERPROFILE" } else { "HOME" }) {
            cmd.cwd(home);
        }

        let default_title = if cfg!(target_os = "windows") { "cmd.exe" } else { "Local Shell" };
        let sess = Self::spawn_from_pair_and_cmd(pair, cmd, rows, cols, default_title, None)?;
        {
            let shell_name = if cfg!(target_os = "windows") { "cmd.exe" } else { "bash" };
            let banner = format!(
                "\x1b[38;5;75m────────────────────────────────────────────────────────────────────────\x1b[0m\r\n\
                 \x1b[1;37m  Terminal :\x1b[0m \x1b[1;36mRustyBox Local Terminal ({})\x1b[0m\r\n\
                 \x1b[32m  ✔\x1b[0m Interactive Shell  \x1b[32m✔\x1b[0m VT100/ANSI Emulation  \x1b[32m✔\x1b[0m UTF-8\r\n\
                 \x1b[38;5;75m────────────────────────────────────────────────────────────────────────\x1b[0m\r\n\r\n",
                shell_name
            );
            let mut p = sess.parser.lock();
            p.process(banner.as_bytes());
        }
        Ok(sess)
    }

    pub fn spawn_ssh(
        rows: u16,
        cols: u16,
        host: &str,
        port: u16,
        user: &str,
        key_path: Option<&str>,
        jump_host: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let mut cmd = CommandBuilder::new("ssh");
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.args(["-p", &port.to_string()]);
        cmd.args(["-o", "StrictHostKeyChecking=accept-new"]);

        if let Some(key) = key_path {
            if !key.trim().is_empty() {
                cmd.args(["-i", key.trim()]);
            }
        }

        if let Some(jh) = jump_host {
            if !jh.trim().is_empty() {
                cmd.args(["-J", jh.trim()]);
            }
        }

        let target = if user.is_empty() {
            host.to_string()
        } else {
            format!("{}@{}", user, host)
        };
        cmd.arg(target);

        let default_title = format!("SSH: {}@{}", user, host);
        let sess = Self::spawn_from_pair_and_cmd(pair, cmd, rows, cols, &default_title, password.map(|p| p.to_string()))?;
        {
            let banner = format!(
                "\x1b[38;5;75m────────────────────────────────────────────────────────────────────────\x1b[0m\r\n\
                 \x1b[1;37m  Session :\x1b[0m \x1b[1;36m{}@{}:{}\x1b[0m\r\n\
                 \x1b[32m  ✔\x1b[0m Direct SSH      \x1b[32m✔\x1b[0m SSH compression\r\n\
                 \x1b[32m  ✔\x1b[0m SSH-browser     \x1b[32m✔\x1b[0m X11-forwarding\r\n\
                 \x1b[38;5;75m────────────────────────────────────────────────────────────────────────\x1b[0m\r\n",
                if user.is_empty() { "root" } else { user },
                host,
                port
            );
            let mut p = sess.parser.lock();
            p.process(banner.as_bytes());
        }
        Ok(sess)
    }

    fn spawn_from_pair_and_cmd(
        pair: PtyPair,
        cmd: CommandBuilder,
        rows: u16,
        cols: u16,
        initial_title: &str,
        auto_password: Option<String>,
    ) -> Result<Self, String> {
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn process: {}", e))?;

        let pid = child.process_id();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;
        let writer_raw = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take writer: {}", e))?;
        let writer = Arc::new(parking_lot::Mutex::new(writer_raw));
        let writer_reply = Arc::clone(&writer);

        let nz_rows = NonZeroU16::new(rows).unwrap_or(NonZeroU16::new(24).unwrap());
        let nz_cols = NonZeroU16::new(cols).unwrap_or(NonZeroU16::new(80).unwrap());
        let parser = Arc::new(parking_lot::Mutex::new(vt100::Parser::new(
            nz_rows, nz_cols, 2000,
        )));
        let running = Arc::new(AtomicBool::new(true));
        let title = Arc::new(parking_lot::Mutex::new(initial_title.to_string()));

        let parser_clone = Arc::clone(&parser);
        let running_clone = Arc::clone(&running);
        let has_new_data = Arc::new(AtomicBool::new(true));
        let has_new_data_clone = Arc::clone(&has_new_data);

        thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                let mut prev_tail: Vec<u8> = Vec::new();
                let mut pending_password = auto_password;
                while running_clone.load(Ordering::Relaxed) {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            running_clone.store(false, Ordering::Relaxed);
                            {
                                let mut p = parser_clone.lock();
                                p.process(b"\r\n\x1b[33m----------------------------------------------------------------\x1b[0m\r\n\x1b[1;31mSession stopped (connection closed).\x1b[0m\r\n\x1b[1;36mPress <Enter> to restart session, or <Ctrl+W> to close tab.\x1b[0m\r\n");
                            }
                            has_new_data_clone.store(true, Ordering::Relaxed);
                            break;
                        }
                        Err(_) => {
                            running_clone.store(false, Ordering::Relaxed);
                            {
                                let mut p = parser_clone.lock();
                                p.process(b"\r\n\x1b[33m----------------------------------------------------------------\x1b[0m\r\n\x1b[1;31mSession stopped (connection closed).\x1b[0m\r\n\x1b[1;36mPress <Enter> to restart session, or <Ctrl+W> to close tab.\x1b[0m\r\n");
                            }
                            has_new_data_clone.store(true, Ordering::Relaxed);
                            break;
                        }
                        Ok(n) => {
                            let chunk = &buf[..n];

                            // ConPTY / VT terminal query handling & auto-reply
                            // ConPTY on Windows blocks startup and resize waiting for CPR (\x1b[6n)
                            let mut combined = prev_tail.clone();
                            combined.extend_from_slice(chunk);

                            // Auto-fill SSH password if prompted
                            if pending_password.is_some() && is_ssh_password_prompt(&combined) {
                                if let Some(pwd) = pending_password.take() {
                                    let writer_for_pwd = Arc::clone(&writer_reply);
                                    let parser_for_notify = Arc::clone(&parser_clone);
                                    let has_new_data_for_notify = Arc::clone(&has_new_data_clone);
                                    thread::spawn(move || {
                                        // Small pause for OpenSSH to configure terminal into no-echo mode
                                        thread::sleep(Duration::from_millis(150));
                                        {
                                            let mut p = parser_for_notify.lock();
                                            p.process(b"\r\n\x1b[1;32m[RustyBox] \xe2\x9c\x94 Auto-submitting saved password...\x1b[0m\r\n");
                                        }
                                        has_new_data_for_notify.store(true, Ordering::Relaxed);

                                        let mut w = writer_for_pwd.lock();
                                        let _ = w.write_all(pwd.as_bytes());
                                        let _ = w.write_all(b"\r");
                                        let _ = w.flush();
                                    });
                                }
                            }

                            // 1. DSR Cursor Position Query (\x1b[6n or \x1b[?6n)
                            if combined.windows(4).any(|w| w == b"\x1b[6n") {
                                let (r, c) = {
                                    let p = parser_clone.lock();
                                    p.screen().cursor_position()
                                };
                                let resp = format!("\x1b[{};{}R", r + 1, c + 1);
                                let mut w = writer_reply.lock();
                                let _ = w.write_all(resp.as_bytes());
                                let _ = w.flush();
                            } else if combined.windows(5).any(|w| w == b"\x1b[?6n") {
                                let (r, c) = {
                                    let p = parser_clone.lock();
                                    p.screen().cursor_position()
                                };
                                let resp = format!("\x1b[?{};{}R", r + 1, c + 1);
                                let mut w = writer_reply.lock();
                                let _ = w.write_all(resp.as_bytes());
                                let _ = w.flush();
                            }

                            // 2. Primary Device Attributes (\x1b[c or \x1b[0c)
                            if combined.windows(3).any(|w| w == b"\x1b[c")
                                || combined.windows(4).any(|w| w == b"\x1b[0c")
                            {
                                let mut w = writer_reply.lock();
                                let _ = w.write_all(b"\x1b[?1;2c");
                                let _ = w.flush();
                            }

                            // 3. Status Report Query (\x1b[5n)
                            if combined.windows(4).any(|w| w == b"\x1b[5n") {
                                let mut w = writer_reply.lock();
                                let _ = w.write_all(b"\x1b[0n");
                                let _ = w.flush();
                            }

                            let tail_len = combined.len().min(256);
                            prev_tail = combined[combined.len() - tail_len..].to_vec();

                            let mut p = parser_clone.lock();
                            p.process(chunk);
                            has_new_data_clone.store(true, Ordering::Relaxed);
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn reader thread: {}", e))?;

        let child_holder = Arc::new(parking_lot::Mutex::new(Some(child)));

        Ok(Self {
            parser,
            writer,
            master: pair.master,
            running,
            title,
            child_pid: pid,
            has_new_data,
            child: child_holder,
        })
    }

    pub fn write_str(&mut self, s: &str) -> std::io::Result<()> {
        let mut w = self.writer.lock();
        w.write_all(s.as_bytes())?;
        w.flush()
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock();
        w.write_all(bytes)?;
        w.flush()
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows > 0 && cols > 0 {
            let _ = self.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
            if let (Some(nz_r), Some(nz_c)) = (NonZeroU16::new(rows), NonZeroU16::new(cols)) {
                let mut p = self.parser.lock();
                p.set_size(nz_r, nz_c);
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn kill(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(ref mut c) = *self.child.lock() {
            let _ = c.kill();
        }
        #[cfg(not(target_os = "windows"))]
        if let Some(pid) = self.child_pid {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
        }
        #[cfg(target_os = "windows")]
        if let Some(pid) = self.child_pid {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .output();
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        self.kill();
    }
}

pub fn is_ssh_password_prompt(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(bytes);
    let mut clean = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_esc = true;
        } else if in_esc {
            if c.is_ascii_alphabetic() || c == 'm' || c == 'K' || c == 'H' || c == 'J' {
                in_esc = false;
            }
        } else {
            clean.push(c);
        }
    }
    let lower = clean.to_lowercase();
    let trimmed = lower.trim_end();
    (trimmed.ends_with(':') || trimmed.ends_with(": "))
        && (trimmed.contains("password") || trimmed.contains("passphrase") || trimmed.contains("mật khẩu"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_password_prompt_detection() {
        assert!(is_ssh_password_prompt(b"root@45.76.150.226's password: "));
        assert!(is_ssh_password_prompt(b"Password: "));
        assert!(is_ssh_password_prompt(b"password:"));
        assert!(is_ssh_password_prompt(b"ruby@51.79.157.8's password: "));
        assert!(is_ssh_password_prompt(b"Enter passphrase for key '/home/user/.ssh/id_rsa': "));
        assert!(is_ssh_password_prompt(b"\x1b[1;33mroot@vps's password: \x1b[0m"));
        assert!(is_ssh_password_prompt("Vui lòng nhập mật khẩu: ".as_bytes()));

        // Negative cases
        assert!(!is_ssh_password_prompt(b"root@ubuntu:~# "));
        assert!(!is_ssh_password_prompt(b"Last login: Sun Nov  1 10:00:00 2026\n"));
        assert!(!is_ssh_password_prompt(b"Please enter your choice [1-5]: "));
    }
}
