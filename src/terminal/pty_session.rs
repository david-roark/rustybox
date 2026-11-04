use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtyPair, PtySize};
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct SshSpawnConfig {
    pub compression: bool,
    pub keepalive: bool,
    pub tunnel_x11: bool,
    pub remote_command: Option<String>,
    pub do_not_exit_after_cmd: bool,
    pub gateway_key: Option<String>,
    pub log_file_path: Option<PathBuf>,
}

/// Builds deterministic OpenSSH CLI arguments from session configuration.
pub fn build_ssh_args(
    host: &str,
    port: u16,
    user: &str,
    key_path: Option<&str>,
    jump_host: Option<&str>,
    cfg: &SshSpawnConfig,
) -> Vec<String> {
    let mut args = vec![
        "-t".to_string(),
        "-p".to_string(),
        port.to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=accept-new".to_string(),
    ];

    if cfg.compression {
        args.push("-C".to_string());
    }
    if cfg.tunnel_x11 {
        args.push("-X".to_string());
    }
    if cfg.keepalive {
        args.push("-o".to_string());
        args.push("ServerAliveInterval=30".to_string());
        args.push("-o".to_string());
        args.push("ServerAliveCountMax=3".to_string());
    }

    if let Some(key) = key_path {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            args.push("-i".to_string());
            args.push(trimmed.to_string());
        }
    }

    if let Some(jh) = jump_host {
        let trimmed = jh.trim();
        if !trimmed.is_empty() {
            if let Some(gw_key) = cfg.gateway_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
                args.push("-o".to_string());
                args.push(format!("ProxyCommand=ssh -i {} -W %h:%p {}", gw_key, trimmed));
            } else {
                args.push("-J".to_string());
                args.push(trimmed.to_string());
            }
        }
    }

    let target = if user.trim().is_empty() {
        host.trim().to_string()
    } else {
        format!("{}@{}", user.trim(), host.trim())
    };
    args.push(target);

    if let Some(cmd) = cfg.remote_command.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        if cfg.do_not_exit_after_cmd {
            args.push(format!("{}; exec ${{SHELL:-/bin/sh}} -l", cmd));
        } else {
            args.push(cmd.to_string());
        }
    }

    args
}

pub struct PtySession {
    pub parser: Arc<parking_lot::Mutex<vt100::Parser>>,
    writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
    master: Box<dyn MasterPty + Send>,
    pub running: Arc<AtomicBool>,
    pub title: Arc<parking_lot::Mutex<String>>,
    pub child_pid: Option<u32>,
    pub has_new_data: Arc<AtomicBool>,
    pub child: Arc<parking_lot::Mutex<Option<Box<dyn portable_pty::Child + Send + Sync>>>>,
    #[allow(dead_code)]
    pub is_ssh: bool,
}

impl PtySession {
    #[allow(dead_code)]
    pub fn spawn_local(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        working_dir: Option<&str>,
    ) -> Result<Self, String> {
        Self::spawn_local_with_log(rows, cols, custom_shell, working_dir, None)
    }

    pub fn spawn_local_with_log(
        rows: u16,
        cols: u16,
        custom_shell: Option<&str>,
        working_dir: Option<&str>,
        log_file_path: Option<PathBuf>,
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
        let sess = Self::spawn_from_pair_and_cmd(
            pair,
            cmd,
            rows,
            cols,
            default_title,
            None,
            false,
            log_file_path,
        )?;
        {
            let shell_name = if cfg!(target_os = "windows") { "cmd.exe" } else { "bash" };
            let banner = format!(
                "\x1b[38;5;75m────────────────────────────────────────────────────\x1b[0m\r\n\
                 \x1b[1;37m  Terminal :\x1b[0m \x1b[1;36mRustyBox Local ({})\x1b[0m\r\n\
                 \x1b[32m  ✔\x1b[0m Shell   \x1b[32m✔\x1b[0m VT100/ANSI   \x1b[32m✔\x1b[0m UTF-8\r\n\
                 \x1b[38;5;75m────────────────────────────────────────────────────\x1b[0m\r\n\r\n",
                shell_name
            );
            let mut p = sess.parser.lock();
            p.process(banner.as_bytes());
        }
        Ok(sess)
    }

    #[allow(dead_code)]
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
        Self::spawn_ssh_with_config(
            rows,
            cols,
            host,
            port,
            user,
            key_path,
            jump_host,
            password,
            &SshSpawnConfig::default(),
        )
    }

    pub fn spawn_ssh_with_config(
        rows: u16,
        cols: u16,
        host: &str,
        port: u16,
        user: &str,
        key_path: Option<&str>,
        jump_host: Option<&str>,
        password: Option<&str>,
        cfg: &SshSpawnConfig,
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

        for arg in build_ssh_args(host, port, user, key_path, jump_host, cfg) {
            cmd.arg(arg);
        }

        if let Some(pwd) = password {
            let clean = pwd.trim();
            if !clean.is_empty() {
                if let Ok(current_exe) = std::env::current_exe() {
                    cmd.env("SSH_ASKPASS", current_exe.to_string_lossy().to_string());
                    cmd.env("SSH_ASKPASS_REQUIRE", "force");
                    cmd.env("DISPLAY", "1");
                    cmd.env("RUSTYBOX_SSH_PASS", clean);
                }
            }
        }

        let default_title = format!("SSH: {}@{}", user, host);
        let sess = Self::spawn_from_pair_and_cmd(
            pair,
            cmd,
            rows,
            cols,
            &default_title,
            password.map(|p| p.to_string()),
            true,
            cfg.log_file_path.clone(),
        )?;
        {
            let comp_badge = if cfg.compression {
                "\x1b[32m✔\x1b[0m Compression"
            } else {
                "\x1b[90m○\x1b[0m Compression"
            };
            let x11_badge = if cfg.tunnel_x11 {
                "\x1b[32m✔\x1b[0m X11 Forward"
            } else {
                "\x1b[90m○\x1b[0m X11 Forward"
            };
            let banner = format!(
                "\x1b[38;5;75m────────────────────────────────────────────────────\x1b[0m\r\n\
                 \x1b[1;37m  Session :\x1b[0m \x1b[1;36m{}@{}:{}\x1b[0m\r\n\
                 \x1b[32m  ✔\x1b[0m Direct SSH      {}\r\n\
                 \x1b[32m  ✔\x1b[0m SFTP Browser    {}\r\n\
                 \x1b[38;5;75m────────────────────────────────────────────────────\x1b[0m\r\n",
                if user.is_empty() { "root" } else { user },
                host,
                port,
                comp_badge,
                x11_badge
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
        is_ssh: bool,
        log_file_path: Option<PathBuf>,
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
        let suppress_terminal_queries = Arc::new(AtomicBool::new(false));
        let suppress_terminal_queries_clone = Arc::clone(&suppress_terminal_queries);
        let mut log_file = log_file_path.and_then(|path| {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        });

        thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                let mut prev_tail: Vec<u8> = Vec::new();
                let mut pending_password = auto_password;
                let mut conpty_init_cpr_replied = false;
                let session_start_time = std::time::Instant::now();
                while running_clone.load(Ordering::Relaxed) {
                    // Security guard: discard pending password after 15s
                    if pending_password.is_some() && session_start_time.elapsed() > Duration::from_secs(15) {
                        pending_password = None;
                    }
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
                            if let Some(ref mut f) = log_file {
                                let _ = f.write_all(chunk);
                                let _ = f.flush();
                            }

                            // ConPTY / VT terminal query handling & auto-reply
                            // ConPTY on Windows blocks startup and resize waiting for CPR (\x1b[6n)
                            let mut combined = prev_tail.clone();
                            combined.extend_from_slice(chunk);

                            // Clear pending password once authenticated shell prompt or post-auth login banner is seen
                            if pending_password.is_some() {
                                let check_str = String::from_utf8_lossy(&combined);
                                if check_str.ends_with("$ ")
                                    || check_str.ends_with("# ")
                                    || check_str.ends_with("% ")
                                    || check_str.contains("Last login:")
                                {
                                    pending_password = None;
                                }
                            }

                            let at_pwd_prompt = is_ssh_password_prompt(&combined);

                            // Auto-fill SSH password if prompted
                            if pending_password.is_some() && at_pwd_prompt {
                                if let Some(pwd) = pending_password.take() {
                                    let writer_for_pwd = Arc::clone(&writer_reply);
                                    let parser_for_notify = Arc::clone(&parser_clone);
                                    let has_new_data_for_notify = Arc::clone(&has_new_data_clone);
                                    let suppress_for_pwd = Arc::clone(&suppress_terminal_queries_clone);
                                    suppress_for_pwd.store(true, Ordering::SeqCst);

                                    thread::spawn(move || {
                                        // 300ms pause for OpenSSH readpassphrase & FlushConsoleInputBuffer to settle
                                        thread::sleep(Duration::from_millis(300));
                                        {
                                            let mut p = parser_for_notify.lock();
                                            p.process(b"\r\n\x1b[1;32m[RustyBox] \xe2\x9c\x94 Auto-submitting saved password...\x1b[0m\r\n");
                                        }
                                        has_new_data_for_notify.store(true, Ordering::Relaxed);

                                        thread::sleep(Duration::from_millis(100));
                                        let clean_pwd = pwd.trim();
                                        let mut w = writer_for_pwd.lock();
                                        // Write keystrokes with slight pacing so Windows ConPTY input buffer never drops bytes
                                        for b in clean_pwd.as_bytes() {
                                            let _ = w.write_all(&[*b]);
                                            let _ = w.flush();
                                            thread::sleep(Duration::from_millis(10));
                                        }

                                        thread::sleep(Duration::from_millis(100));
                                        let _ = w.write_all(b"\r");
                                        let _ = w.flush();

                                        // Keep queries suppressed for another 800ms while SSH server authenticates
                                        thread::sleep(Duration::from_millis(800));
                                        suppress_for_pwd.store(false, Ordering::SeqCst);
                                    });
                                }
                            }

                            // On Windows, ConPTY (conhost.exe --headless --inheritcursor) emits \x1b[6n
                            // immediately upon pseudo-console creation and blocks the child process (ssh.exe)
                            // from starting until it receives a Cursor Position Report (\x1b[r;cR).
                            // Reply to this initial ConPTY handshake once so ssh.exe can start, then block
                            // subsequent terminal queries on SSH sessions so remote shells never see ^[[6;1R.
                            if is_ssh && !conpty_init_cpr_replied {
                                if let Some(pos) = combined.windows(4).position(|w| w == b"\x1b[6n") {
                                    conpty_init_cpr_replied = true;
                                    let (r, c) = {
                                        let p = parser_clone.lock();
                                        p.screen().cursor_position()
                                    };
                                    let resp = format!("\x1b[{};{}R", r + 1, c + 1);
                                    let mut w = writer_reply.lock();
                                    let _ = w.write_all(resp.as_bytes());
                                    let _ = w.flush();
                                    combined[pos..pos + 4].fill(0);
                                }
                            }

                            // If this is an SSH session, NEVER send subsequent terminal query auto-replies (CPR, DA, DSR)
                            // into stdin, because OpenSSH / remote shell (bash/zsh) will treat them as raw
                            // user keystrokes, printing "^[[6;1R" and leaving ";1R" at the command prompt!
                            // Also block queries during password auto-injection.
                            let queries_blocked = is_ssh || at_pwd_prompt || suppress_terminal_queries_clone.load(Ordering::SeqCst);

                            if !queries_blocked {
                                // 1. DSR Cursor Position Query (\x1b[6n or \x1b[?6n)
                                if let Some(pos) = combined.windows(4).position(|w| w == b"\x1b[6n") {
                                    let (r, c) = {
                                        let p = parser_clone.lock();
                                        p.screen().cursor_position()
                                    };
                                    let resp = format!("\x1b[{};{}R", r + 1, c + 1);
                                    let mut w = writer_reply.lock();
                                    let _ = w.write_all(resp.as_bytes());
                                    let _ = w.flush();
                                    combined[pos..pos + 4].fill(0);
                                } else if let Some(pos) = combined.windows(5).position(|w| w == b"\x1b[?6n") {
                                    let (r, c) = {
                                        let p = parser_clone.lock();
                                        p.screen().cursor_position()
                                    };
                                    let resp = format!("\x1b[?{};{}R", r + 1, c + 1);
                                    let mut w = writer_reply.lock();
                                    let _ = w.write_all(resp.as_bytes());
                                    let _ = w.flush();
                                    combined[pos..pos + 5].fill(0);
                                }

                                // 2. Primary Device Attributes (\x1b[c or \x1b[0c)
                                if let Some(pos) = combined.windows(3).position(|w| w == b"\x1b[c") {
                                    let mut w = writer_reply.lock();
                                    let _ = w.write_all(b"\x1b[?1;2c");
                                    let _ = w.flush();
                                    combined[pos..pos + 3].fill(0);
                                } else if let Some(pos) = combined.windows(4).position(|w| w == b"\x1b[0c") {
                                    let mut w = writer_reply.lock();
                                    let _ = w.write_all(b"\x1b[?1;2c");
                                    let _ = w.flush();
                                    combined[pos..pos + 4].fill(0);
                                }

                                // 3. Status Report Query (\x1b[5n)
                                if let Some(pos) = combined.windows(4).position(|w| w == b"\x1b[5n") {
                                    let mut w = writer_reply.lock();
                                    let _ = w.write_all(b"\x1b[0n");
                                    let _ = w.flush();
                                    combined[pos..pos + 4].fill(0);
                                }
                            }

                            let tail_len = if pending_password.is_some() {
                                combined.len().min(256)
                            } else {
                                combined.len().min(8)
                            };
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
            is_ssh,
        })
    }

    #[allow(dead_code)]
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

    /// Send SIGINT (Ctrl+C) to the foreground process group via PTY master (\x03)
    pub fn send_sigint(&mut self) -> std::io::Result<()> {
        self.write_bytes(b"\x03")
    }

    /// Pastes text into the PTY, snapping scrollback to 0 and wrapping in Bracketed Paste
    /// (`\x1b[200~...\x1b[201~`) if the active shell/TUI has enabled bracketed paste mode.
    pub fn paste_text(&mut self, text: &str) -> std::io::Result<Vec<u8>> {
        let bracketed = {
            let mut p = self.parser.lock();
            p.screen_mut().set_scrollback(0);
            p.screen().bracketed_paste()
        };
        let bytes = if bracketed {
            let mut out = Vec::with_capacity(text.len() + 12);
            out.extend_from_slice(b"\x1b[200~");
            out.extend_from_slice(text.as_bytes());
            out.extend_from_slice(b"\x1b[201~");
            out
        } else {
            text.as_bytes().to_vec()
        };
        self.write_bytes(&bytes)?;
        Ok(bytes)
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
        if !self.running.load(Ordering::Relaxed) {
            return false;
        }
        if let Some(mut guard) = self.child.try_lock() {
            if let Some(child) = guard.as_mut() {
                if let Ok(Some(_status)) = child.try_wait() {
                    self.running.store(false, Ordering::Relaxed);
                    return false;
                }
            }
        }
        self.running.load(Ordering::Relaxed)
    }

    pub fn kill(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(mut c) = self.child.lock().take() {
            let _ = c.kill();
        }
        if let Some(pid) = self.child_pid.take() {
            std::thread::spawn(move || {
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/T", "/PID", &pid.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
            });
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_send_sigint_interrupts_foreground_process() {
        let mut sess = PtySession::spawn_local(24, 80, Some("/bin/sh"), None)
            .expect("Failed to spawn local PTY session");
        std::thread::sleep(Duration::from_millis(120));

        // Start a foreground loop simulating `docker logs -f` that traps SIGINT or exits on Ctrl+C,
        // then run an echo command after Ctrl+C to prove the shell recovered control.
        sess.write_str("sh -c 'trap \"echo SIGINT_CAUGHT; exit 0\" INT; echo STREAM_READY; while :; do sleep 1; done'\n")
            .expect("Failed to write command");

        let mut ready = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(50));
            let contents = sess.parser.lock().screen().contents();
            if contents.contains("STREAM_READY") {
                ready = true;
                break;
            }
        }
        assert!(ready, "Foreground streaming process did not start");

        // Send Ctrl+C (SIGINT)
        sess.send_sigint().expect("Failed to send SIGINT");

        let mut interrupted = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(50));
            let contents = sess.parser.lock().screen().contents();
            if contents.contains("SIGINT_CAUGHT") {
                interrupted = true;
                break;
            }
        }
        assert!(
            interrupted,
            "Foreground process was not interrupted by Ctrl+C (send_sigint)"
        );
        assert!(
            sess.is_alive(),
            "Parent shell should remain alive after interrupting foreground command"
        );
        sess.kill();
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_pty_ctrl_c_cancels_partial_input_and_resize_updates_screen() {
        let mut sess = PtySession::spawn_local(24, 80, Some("/bin/sh"), None)
            .expect("Failed to spawn local PTY session");
        std::thread::sleep(Duration::from_millis(100));

        // Resize terminal to 40x120 and verify parser size updates
        sess.resize(40, 120);
        {
            let p = sess.parser.lock();
            let (r, c) = p.screen().size();
            assert_eq!(r.get(), 40);
            assert_eq!(c.get(), 120);
        }

        // Write a partial command WITHOUT newline, then send Ctrl+C to cancel it, then run echo
        sess.write_str("echo SHOULD_NOT_RUN_")
            .expect("Failed to write partial command");
        std::thread::sleep(Duration::from_millis(50));
        sess.send_sigint().expect("Failed to send SIGINT");
        std::thread::sleep(Duration::from_millis(50));
        sess.write_str("echo AFTER_SIGINT_OK\n")
            .expect("Failed to write post-sigint command");

        let mut found_after = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(50));
            let contents = sess.parser.lock().screen().contents();
            if contents.contains("AFTER_SIGINT_OK") {
                found_after = true;
                break;
            }
        }
        assert!(found_after, "Command after Ctrl+C did not execute");
        sess.kill();
    }

    #[test]
    fn test_build_ssh_args_advanced_options() {
        let cfg = SshSpawnConfig {
            compression: true,
            keepalive: true,
            tunnel_x11: true,
            remote_command: Some("tmux attach || tmux new".to_string()),
            do_not_exit_after_cmd: true,
            gateway_key: None,
            log_file_path: None,
        };
        let args = build_ssh_args(
            "10.0.0.5",
            2222,
            "deploy",
            Some("/home/user/.ssh/id_ed25519"),
            Some("bastion@1.2.3.4:22"),
            &cfg,
        );
        assert!(args.contains(&"-C".to_string()));
        assert!(args.contains(&"-X".to_string()));
        assert!(args.contains(&"ServerAliveInterval=30".to_string()));
        assert!(args.contains(&"-J".to_string()));
        assert!(args.contains(&"bastion@1.2.3.4:22".to_string()));
        assert!(args.contains(&"deploy@10.0.0.5".to_string()));
        assert_eq!(
            args.last().unwrap(),
            "tmux attach || tmux new; exec ${SHELL:-/bin/sh} -l"
        );

        // With gateway_key set, ProxyCommand should be used instead of -J
        let cfg_gw_key = SshSpawnConfig {
            gateway_key: Some("/home/user/.ssh/bastion_key".to_string()),
            ..cfg
        };
        let args_gw = build_ssh_args(
            "10.0.0.5",
            22,
            "root",
            None,
            Some("ops@bastion.local:22"),
            &cfg_gw_key,
        );
        assert!(args_gw
            .iter()
            .any(|a| a.starts_with("ProxyCommand=ssh -i /home/user/.ssh/bastion_key")));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_paste_text_bracketed_and_output_logging() {
        let log_path = std::env::temp_dir().join(format!(
            "rustybox_pty_log_{}.log",
            uuid::Uuid::new_v4()
        ));
        let mut sess =
            PtySession::spawn_local_with_log(24, 80, Some("/bin/sh"), None, Some(log_path.clone()))
                .expect("Failed to spawn local PTY with log");
        std::thread::sleep(Duration::from_millis(80));

        // Enable bracketed paste mode (\x1b[?2004h) on parser and verify paste_text wraps with \x1b[200~..\x1b[201~
        sess.parser.lock().process(b"\x1b[?2004h");
        let sent = sess
            .paste_text("echo LOG_TEST_OK\n")
            .expect("Failed to paste text");
        assert!(sent.starts_with(b"\x1b[200~"));
        assert!(sent.ends_with(b"\x1b[201~"));

        // Also send plain newline command so shell executes and writes to log file
        sess.write_str("\necho LOG_FILE_VERIFIED\n").unwrap();
        let mut logged = false;
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(40));
            if let Ok(data) = std::fs::read_to_string(&log_path) {
                if data.contains("LOG_FILE_VERIFIED") {
                    logged = true;
                    break;
                }
            }
        }
        sess.kill();
        let _ = std::fs::remove_file(&log_path);
        assert!(logged, "Session output was not written to log_file_path");
    }
}
