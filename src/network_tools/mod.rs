use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

#[derive(Clone, Debug)]
pub struct ScanResult {
    pub port: u16,
    pub service: &'static str,
    pub is_open: bool,
    pub latency_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct SubnetHostResult {
    pub ip: String,
    pub is_alive: bool,
    pub latency_ms: Option<u64>,
    pub open_service: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct NetstatEntry {
    pub proto: String,
    pub local_addr: String,
    pub remote_addr: String,
    pub state: String,
}

pub struct NetworkTools {
    // Port Scanner
    pub target_host: String,
    pub is_scanning: Arc<AtomicBool>,
    pub scan_results: Arc<parking_lot::Mutex<Vec<ScanResult>>>,
    pub scan_progress: Arc<parking_lot::Mutex<f32>>,
    // Subnet / LAN IP Scanner (MobaXterm NetScan)
    pub subnet_prefix: String,
    pub is_subnet_scanning: Arc<AtomicBool>,
    pub subnet_results: Arc<parking_lot::Mutex<Vec<SubnetHostResult>>>,
    pub subnet_progress: Arc<parking_lot::Mutex<f32>>,
    // Ping Tool
    pub ping_host: String,
    pub is_pinging: Arc<AtomicBool>,
    pub ping_logs: Arc<parking_lot::Mutex<Vec<String>>>,
    // Netstat Viewer (MobaXterm TFORMNETSTAT)
    pub netstat_entries: Arc<parking_lot::Mutex<Vec<NetstatEntry>>>,
    pub is_netstat_loading: Arc<AtomicBool>,
    // Micro HTTP Server (MobaXterm TFORMSERVICES HTTP Daemon)
    pub http_port: u16,
    pub http_root: PathBuf,
    pub http_running: Arc<AtomicBool>,
    pub http_logs: Arc<parking_lot::Mutex<Vec<String>>>,
    // Micro Syslog Server (UDP port 1514 / 514)
    pub syslog_port: u16,
    pub syslog_running: Arc<AtomicBool>,
    pub syslog_logs: Arc<parking_lot::Mutex<Vec<String>>>,
}

impl NetworkTools {
    pub fn new() -> Self {
        let initial_root = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            target_host: "127.0.0.1".to_string(),
            is_scanning: Arc::new(AtomicBool::new(false)),
            scan_results: Arc::new(parking_lot::Mutex::new(Vec::new())),
            scan_progress: Arc::new(parking_lot::Mutex::new(0.0)),
            subnet_prefix: "192.168.1".to_string(),
            is_subnet_scanning: Arc::new(AtomicBool::new(false)),
            subnet_results: Arc::new(parking_lot::Mutex::new(Vec::new())),
            subnet_progress: Arc::new(parking_lot::Mutex::new(0.0)),
            ping_host: "8.8.8.8".to_string(),
            is_pinging: Arc::new(AtomicBool::new(false)),
            ping_logs: Arc::new(parking_lot::Mutex::new(Vec::new())),
            netstat_entries: Arc::new(parking_lot::Mutex::new(Vec::new())),
            is_netstat_loading: Arc::new(AtomicBool::new(false)),
            http_port: 8080,
            http_root: initial_root,
            http_running: Arc::new(AtomicBool::new(false)),
            http_logs: Arc::new(parking_lot::Mutex::new(Vec::new())),
            syslog_port: 1514,
            syslog_running: Arc::new(AtomicBool::new(false)),
            syslog_logs: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    pub fn start_port_scan(&self, runtime: &tokio::runtime::Handle) {
        if self.is_scanning.load(Ordering::Relaxed) {
            return;
        }

        self.is_scanning.store(true, Ordering::Relaxed);
        *self.scan_progress.lock() = 0.0;
        self.scan_results.lock().clear();

        let host = self.target_host.clone();
        let is_scanning = Arc::clone(&self.is_scanning);
        let scan_results = Arc::clone(&self.scan_results);
        let progress = Arc::clone(&self.scan_progress);

        runtime.spawn(async move {
            let common_ports: &[(u16, &'static str)] = &[
                (21, "FTP"),
                (22, "SSH"),
                (23, "Telnet"),
                (25, "SMTP"),
                (53, "DNS"),
                (80, "HTTP"),
                (110, "POP3"),
                (143, "IMAP"),
                (443, "HTTPS"),
                (445, "SMB"),
                (1433, "MSSQL"),
                (1521, "Oracle"),
                (3306, "MySQL"),
                (3389, "RDP"),
                (5432, "PostgreSQL"),
                (5900, "VNC"),
                (6379, "Redis"),
                (8000, "HTTP-Alt"),
                (8080, "HTTP-Proxy"),
                (8443, "HTTPS-Alt"),
                (9200, "Elasticsearch"),
                (27017, "MongoDB"),
            ];

            let total = common_ports.len() as f32;
            for (idx, &(port, service)) in common_ports.iter().enumerate() {
                if !is_scanning.load(Ordering::Relaxed) {
                    break;
                }

                let target = format!("{}:{}", host, port);
                let start = Instant::now();
                let is_open = match tokio::time::timeout(
                    Duration::from_millis(400),
                    TcpStream::connect(&target),
                )
                .await
                {
                    Ok(Ok(_)) => true,
                    _ => false,
                };
                let latency = if is_open {
                    Some(start.elapsed().as_millis() as u64)
                } else {
                    None
                };

                scan_results.lock().push(ScanResult {
                    port,
                    service,
                    is_open,
                    latency_ms: latency,
                });

                *progress.lock() = (idx + 1) as f32 / total;
            }

            is_scanning.store(false, Ordering::Relaxed);
        });
    }

    pub fn start_subnet_scan(&self, runtime: &tokio::runtime::Handle) {
        if self.is_subnet_scanning.load(Ordering::Relaxed) {
            return;
        }

        self.is_subnet_scanning.store(true, Ordering::Relaxed);
        *self.subnet_progress.lock() = 0.0;
        self.subnet_results.lock().clear();

        let prefix = self.subnet_prefix.trim().trim_end_matches('.').to_string();
        let is_scanning = Arc::clone(&self.is_subnet_scanning);
        let results = Arc::clone(&self.subnet_results);
        let progress = Arc::clone(&self.subnet_progress);

        runtime.spawn(async move {
            // Scan first 50 common hosts in subnet fast
            let total = 50.0;
            for host_id in 1..=50 {
                if !is_scanning.load(Ordering::Relaxed) {
                    break;
                }

                let ip = format!("{}.{}", prefix, host_id);
                // Check if port 80, 22, 445 or 443 responds
                let mut found = false;
                let mut resp_service = None;
                let start = Instant::now();

                for &(port, name) in &[(80, "HTTP"), (22, "SSH"), (445, "SMB"), (443, "HTTPS")] {
                    let target = format!("{}:{}", ip, port);
                    if let Ok(Ok(_)) = tokio::time::timeout(
                        Duration::from_millis(150),
                        TcpStream::connect(&target),
                    )
                    .await
                    {
                        found = true;
                        resp_service = Some(name);
                        break;
                    }
                }

                if found {
                    let lat = start.elapsed().as_millis() as u64;
                    results.lock().push(SubnetHostResult {
                        ip,
                        is_alive: true,
                        latency_ms: Some(lat),
                        open_service: resp_service,
                    });
                }

                *progress.lock() = host_id as f32 / total;
            }

            is_scanning.store(false, Ordering::Relaxed);
        });
    }

    pub fn refresh_netstat(&self, runtime: &tokio::runtime::Handle) {
        if self.is_netstat_loading.load(Ordering::Relaxed) {
            return;
        }

        self.is_netstat_loading.store(true, Ordering::Relaxed);
        let entries_arc = Arc::clone(&self.netstat_entries);
        let loading = Arc::clone(&self.is_netstat_loading);

        runtime.spawn(async move {
            let mut list = Vec::new();

            // Try reading /proc/net/tcp on Linux
            if let Ok(content) = tokio::fs::read_to_string("/proc/net/tcp").await {
                for line in content.lines().skip(1).take(40) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let local = parse_proc_addr(parts[1]);
                        let remote = parse_proc_addr(parts[2]);
                        let state = match parts[3] {
                            "01" => "ESTABLISHED",
                            "02" => "SYN_SENT",
                            "03" => "SYN_RECV",
                            "04" => "FIN_WAIT1",
                            "05" => "FIN_WAIT2",
                            "06" => "TIME_WAIT",
                            "07" => "CLOSE",
                            "08" => "CLOSE_WAIT",
                            "09" => "LAST_ACK",
                            "0A" => "LISTEN",
                            "0B" => "CLOSING",
                            _ => "UNKNOWN",
                        };
                        list.push(NetstatEntry {
                            proto: "TCP".into(),
                            local_addr: local,
                            remote_addr: remote,
                            state: state.into(),
                        });
                    }
                }
            } else {
                // Fallback demo/active sockets for Windows or non-proc
                list.push(NetstatEntry {
                    proto: "TCP".into(),
                    local_addr: "0.0.0.0:22".into(),
                    remote_addr: "0.0.0.0:0".into(),
                    state: "LISTEN".into(),
                });
                list.push(NetstatEntry {
                    proto: "TCP".into(),
                    local_addr: "127.0.0.1:8080".into(),
                    remote_addr: "0.0.0.0:0".into(),
                    state: "LISTEN".into(),
                });
            }

            *entries_arc.lock() = list;
            loading.store(false, Ordering::Relaxed);
        });
    }

    pub fn start_ping(&self, runtime: &tokio::runtime::Handle) {
        if self.is_pinging.load(Ordering::Relaxed) {
            return;
        }

        self.is_pinging.store(true, Ordering::Relaxed);
        self.ping_logs.lock().clear();

        let host = self.ping_host.clone();
        let is_pinging = Arc::clone(&self.is_pinging);
        let logs = Arc::clone(&self.ping_logs);

        runtime.spawn(async move {
            let port = 80;
            let target = format!("{}:{}", host, port);
            logs.lock().push(format!("Starting TCP ping to {} on port {}...", host, port));

            for seq in 1..=10 {
                if !is_pinging.load(Ordering::Relaxed) {
                    break;
                }

                let start = Instant::now();
                let res = tokio::time::timeout(
                    Duration::from_millis(1500),
                    TcpStream::connect(&target),
                )
                .await;

                let elapsed = start.elapsed().as_millis();
                match res {
                    Ok(Ok(_)) => {
                        logs.lock().push(format!(
                            "seq={} from {}: time={}ms status=Connected",
                            seq, host, elapsed
                        ));
                    }
                    Ok(Err(e)) => {
                        logs.lock().push(format!(
                            "seq={} from {}: time={}ms error={}",
                            seq, host, elapsed, e
                        ));
                    }
                    Err(_) => {
                        logs.lock().push(format!("seq={} from {}: Request timed out", seq, host));
                    }
                }

                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            logs.lock().push("Ping finished.".to_string());
            is_pinging.store(false, Ordering::Relaxed);
        });
    }

    pub fn start_http_server(&self, runtime: &tokio::runtime::Handle) {
        if self.http_running.load(Ordering::Relaxed) {
            return;
        }

        self.http_running.store(true, Ordering::Relaxed);
        self.http_logs.lock().clear();

        let running = Arc::clone(&self.http_running);
        let logs = Arc::clone(&self.http_logs);
        let port = self.http_port;
        let root = self.http_root.clone();

        runtime.spawn(async move {
            let addr = format!("0.0.0.0:{}", port);
            match TcpListener::bind(&addr).await {
                Ok(listener) => {
                    logs.lock().push(format!("🚀 HTTP Server started on http://127.0.0.1:{}", port));
                    logs.lock().push(format!("📁 Serving root: {}", root.display()));

                    while running.load(Ordering::Relaxed) {
                        tokio::select! {
                            accept_res = listener.accept() => {
                                if let Ok((mut socket, peer)) = accept_res {
                                    let root_dir = root.clone();
                                    let logs_inner = Arc::clone(&logs);
                                    tokio::spawn(async move {
                                        let mut buf = [0u8; 4096];
                                        if let Ok(n) = socket.read(&mut buf).await {
                                            if n > 0 {
                                                let req_str = String::from_utf8_lossy(&buf[..n]);
                                                let first_line = req_str.lines().next().unwrap_or("");
                                                let parts: Vec<&str> = first_line.split_whitespace().collect();
                                                let path = if parts.len() > 1 { parts[1] } else { "/" };

                                                logs_inner.lock().push(format!("Peer {} -> {} {}", peer, parts.first().unwrap_or(&""), path));

                                                let decoded_path = percent_decode(path);
                                                let target = if decoded_path == "/" {
                                                    root_dir.clone()
                                                } else {
                                                    root_dir.join(decoded_path.trim_start_matches('/'))
                                                };

                                                if target.is_dir() {
                                                    let mut html = format!("<html><head><title>RustyBox File Server</title><meta charset='utf-8'></head><body style='font-family:sans-serif;background:#1e1e24;color:#eee;padding:20px;'><h2>📁 Index of {}</h2><hr><ul>", path);
                                                    if let Ok(entries) = std::fs::read_dir(&target) {
                                                        for entry in entries.flatten() {
                                                            let name = entry.file_name().to_string_lossy().to_string();
                                                            let is_dir = entry.path().is_dir();
                                                            let slash = if is_dir { "/" } else { "" };
                                                            let href = if path.ends_with('/') {
                                                                format!("{}{}{}", path, name, slash)
                                                            } else {
                                                                format!("{}/{}{}", path, name, slash)
                                                            };
                                                            let icon = if is_dir { "📁" } else { "📄" };
                                                            html.push_str(&format!("<li>{} <a style='color:#68d391;text-decoration:none;' href='{}'>{}{}</a></li>", icon, href, name, slash));
                                                        }
                                                    }
                                                    html.push_str("</ul><hr><p style='color:#777;font-size:12px;'>Powered by RustyBox Portable HTTP Daemon</p></body></html>");
                                                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", html.len(), html);
                                                    let _ = socket.write_all(response.as_bytes()).await;
                                                } else if target.is_file() {
                                                    if let Ok(data) = tokio::fs::read(&target).await {
                                                        let header = format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", data.len());
                                                        let _ = socket.write_all(header.as_bytes()).await;
                                                        let _ = socket.write_all(&data).await;
                                                    } else {
                                                        let _ = socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n").await;
                                                    }
                                                } else {
                                                    let not_found = "404 Not Found";
                                                    let response = format!("HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n{}", not_found.len(), not_found);
                                                    let _ = socket.write_all(response.as_bytes()).await;
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                        }
                    }
                }
                Err(e) => {
                    logs.lock().push(format!("❌ Failed to bind HTTP server to port {}: {}", port, e));
                    running.store(false, Ordering::Relaxed);
                }
            }
        });
    }

    pub fn stop_http_server(&self) {
        self.http_running.store(false, Ordering::Relaxed);
        self.http_logs.lock().push("🛑 HTTP Server stopped.".to_string());
    }

    pub fn start_syslog_server(&self, runtime: &tokio::runtime::Handle) {
        if self.syslog_running.load(Ordering::Relaxed) {
            return;
        }

        self.syslog_running.store(true, Ordering::Relaxed);
        self.syslog_logs.lock().clear();

        let running = Arc::clone(&self.syslog_running);
        let logs = Arc::clone(&self.syslog_logs);
        let port = self.syslog_port;

        runtime.spawn(async move {
            let addr = format!("0.0.0.0:{}", port);
            match UdpSocket::bind(&addr).await {
                Ok(socket) => {
                    logs.lock().push(format!("📡 Syslog Daemon listening on UDP 0.0.0.0:{}", port));
                    let mut buf = [0u8; 2048];

                    while running.load(Ordering::Relaxed) {
                        tokio::select! {
                            res = socket.recv_from(&mut buf) => {
                                if let Ok((len, peer)) = res {
                                    let msg = String::from_utf8_lossy(&buf[..len]);
                                    logs.lock().push(format!("[{}] {}", peer, msg.trim()));
                                }
                            }
                            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                        }
                    }
                }
                Err(e) => {
                    logs.lock().push(format!("❌ Failed to bind Syslog to UDP {}: {}", port, e));
                    running.store(false, Ordering::Relaxed);
                }
            }
        });
    }

    pub fn stop_syslog_server(&self) {
        self.syslog_running.store(false, Ordering::Relaxed);
        self.syslog_logs.lock().push("🛑 Syslog Daemon stopped.".to_string());
    }
}

fn percent_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                result.push(val);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

fn parse_proc_addr(hex_str: &str) -> String {
    let parts: Vec<&str> = hex_str.split(':').collect();
    if parts.len() != 2 {
        return hex_str.to_string();
    }
    if let (Ok(ip_num), Ok(port)) = (u32::from_str_radix(parts[0], 16), u16::from_str_radix(parts[1], 16)) {
        let b1 = (ip_num & 0xFF) as u8;
        let b2 = ((ip_num >> 8) & 0xFF) as u8;
        let b3 = ((ip_num >> 16) & 0xFF) as u8;
        let b4 = ((ip_num >> 24) & 0xFF) as u8;
        format!("{}.{}.{}.{}:{}", b1, b2, b3, b4, port)
    } else {
        hex_str.to_string()
    }
}
