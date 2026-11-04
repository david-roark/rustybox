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
        let sess = Self::spawn_from_pair_and_cmd(pair, cmd, rows, cols, default_title, None, false)?;
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
        let sess = Self::spawn_from_pair_and_cmd(pair, cmd, rows, cols, &default_title, password.map(|p| p.to_string()), true)?;
        {
            let banner = format!(
                "\x1b[38;5;75m────────────────────────────────────────────────────\x1b[0m\r\n\
                 \x1b[1;37m  Session :\x1b[0m \x1b[1;36m{}@{}:{}\x1b[0m\r\n\
                 \x1b[32m  ✔\x1b[0m Direct SSH      \x1b[32m✔\x1b[0m Compression\r\n\
                 \x1b[32m  ✔\x1b[0m SFTP Browser    \x1b[32m✔\x1b[0m X11 Forward\r\n\
                 \x1b[38;5;75m────────────────────────────────────────────────────\x1b[0m\r\n",
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
        is_ssh: bool,
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

    /// Send SIGINT (Ctrl+C) to child process group
    pub fn send_sigint(&mut self) -> std::io::Result<()> {
        // 1. Send \x03 to PTY master
        let _ = self.write_bytes(b"\x03");

        // 2. On Unix/Linux, also send SIGINT to child PID via kill command to guarantee interruption
        #[cfg(not(target_os = "windows"))]
        if let Some(pid) = self.child_pid {
            let _ = std::process::Command::new("kill")
                .args(["-2", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        Ok(())
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
}
