use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

#[derive(Clone, Debug)]
pub struct ScanResult {
    pub port: u16,
    pub service: &'static str,
    pub is_open: bool,
    pub latency_ms: Option<u64>,
}

pub struct NetworkTools {
    pub target_host: String,
    pub is_scanning: Arc<AtomicBool>,
    pub scan_results: Arc<parking_lot::Mutex<Vec<ScanResult>>>,
    pub scan_progress: Arc<parking_lot::Mutex<f32>>,
    // Ping Tool
    pub ping_host: String,
    pub is_pinging: Arc<AtomicBool>,
    pub ping_logs: Arc<parking_lot::Mutex<Vec<String>>>,
}

impl NetworkTools {
    pub fn new() -> Self {
        Self {
            target_host: "127.0.0.1".to_string(),
            is_scanning: Arc::new(AtomicBool::new(false)),
            scan_results: Arc::new(parking_lot::Mutex::new(Vec::new())),
            scan_progress: Arc::new(parking_lot::Mutex::new(0.0)),
            ping_host: "8.8.8.8".to_string(),
            is_pinging: Arc::new(AtomicBool::new(false)),
            ping_logs: Arc::new(parking_lot::Mutex::new(Vec::new())),
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
                    Duration::from_millis(500),
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
            let port = 80; // TCP Ping to port 80 or 443
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
}
