use std::time::Instant;

use crate::session::Session;

/// The single shell probe command executed on remote servers.
/// It collects:
/// 1. Cores, 1-min load average, Uptime
/// 2. RAM Total and Available (kB)
/// 3. Disk percentage on root (/)
/// 4. Network interface byte counters
pub const TELEMETRY_PROBE_CMD: &str = r#"printf "CORES:%s|LOAD:%s|UPTIME:%s\n" "$(nproc 2>/dev/null)" "$(cat /proc/loadavg 2>/dev/null | cut -d' ' -f1)" "$(uptime -p 2>/dev/null)"; grep -E "MemTotal|MemAvailable" /proc/meminfo 2>/dev/null; df -k / 2>/dev/null | tail -1; cat /proc/net/dev 2>/dev/null"#;

/// Structured system metrics obtained from telemetry collection
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteSystemMetrics {
    pub cores: Option<u32>,
    pub cpu_pct: Option<u32>,
    pub ram_used_gb: Option<f32>,
    pub ram_total_gb: Option<f32>,
    pub ram_pct: Option<u32>,
    pub uptime: Option<String>,
    pub uptime_secs: Option<u64>,
    pub disk_pct: Option<u32>,
    pub net_rx_bytes: Option<u64>,
    pub net_tx_bytes: Option<u64>,
    pub net_rate: Option<String>,
}

/// Tab telemetry displayed in the bottom telemetry bar
#[derive(Clone, Debug, PartialEq)]
pub struct TabTelemetry {
    pub host: String,
    pub user: String,
    pub cores: Option<u32>,
    pub cpu_pct: Option<u32>,
    pub ram_used_gb: Option<f32>,
    pub ram_total_gb: Option<f32>,
    pub ram_pct: Option<u32>,
    pub uptime: Option<String>,
    pub uptime_secs: Option<u64>,
    pub disk_pct: Option<u32>,
    pub net_rate: Option<String>,
    pub is_remote: bool,
    pub is_connected: bool,
    pub last_updated: Instant,
}

impl TabTelemetry {
    /// Creates a local telemetry state with system hostname/user and safe initial placeholders
    pub fn new_local() -> Self {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .ok();
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
            cpu_pct: None,
            ram_used_gb: None,
            ram_total_gb: None,
            ram_pct: None,
            uptime: None,
            uptime_secs: None,
            disk_pct: read_local_disk_pct(),
            net_rate: None,
            is_remote: false,
            is_connected: true,
            last_updated: Instant::now() - std::time::Duration::from_secs(10),
        }
    }

    /// Creates a remote telemetry state with safe initial placeholders (`--`)
    pub fn new_remote(session: &Session) -> Self {
        let (host_str, user_str) = session.session_type.remote_host_and_user();

        Self {
            host: if !session.name.is_empty() { session.name.clone() } else { host_str },
            user: if !user_str.is_empty() { user_str } else { "root".into() },
            cores: None,
            cpu_pct: None,
            ram_used_gb: None,
            ram_total_gb: None,
            ram_pct: None,
            uptime: None,
            uptime_secs: None,
            disk_pct: None,
            net_rate: None,
            is_remote: true,
            is_connected: true,
            last_updated: Instant::now(),
        }
    }

    /// Updates metrics from collected RemoteSystemMetrics
    pub fn apply_metrics(&mut self, metrics: &RemoteSystemMetrics) {
        if let Some(c) = metrics.cores { self.cores = Some(c); }
        if let Some(cpu) = metrics.cpu_pct { self.cpu_pct = Some(cpu); }
        if let Some(used) = metrics.ram_used_gb { self.ram_used_gb = Some(used); }
        if let Some(total) = metrics.ram_total_gb { self.ram_total_gb = Some(total); }
        if let Some(pct) = metrics.ram_pct { self.ram_pct = Some(pct); }
        if let Some(ref up) = metrics.uptime { self.uptime = Some(up.clone()); }
        if let Some(up_s) = metrics.uptime_secs { self.uptime_secs = Some(up_s); }
        if let Some(d) = metrics.disk_pct { self.disk_pct = Some(d); }
        if let Some(ref net) = metrics.net_rate { self.net_rate = Some(net.clone()); }
        self.last_updated = Instant::now();
    }
}

/// Formats network speed in human-readable notation (e.g. 254.0K, 1.25M)
pub fn format_net_speed(bps: f32) -> String {
    if bps >= 1024.0 * 1024.0 {
        format!("{:.2}M", bps / (1024.0 * 1024.0))
    } else {
        format!("{:.1}K", (bps / 1024.0).max(0.0))
    }
}

/// Formats uptime string nicely for status bar display
pub fn format_uptime_string(raw: &str) -> String {
    let clean = raw.trim();
    let lower = clean.to_lowercase();
    let without_up = if lower.starts_with("up ") {
        &clean[3..].trim()
    } else {
        clean
    };

    // Parse components: e.g. "19 hours, 11 minutes" or "2 days, 4 hours" or "26 minutes"
    let mut days = 0u32;
    let mut hours = 0u32;
    let mut minutes = 0u32;
    let mut has_parts = false;

    for part in without_up.split(',') {
        let p = part.trim();
        let tokens: Vec<&str> = p.split_whitespace().collect();
        if tokens.len() >= 2 {
            if let Ok(n) = tokens[0].parse::<u32>() {
                let unit = tokens[1].to_lowercase();
                if unit.starts_with("day") {
                    days = n;
                    has_parts = true;
                } else if unit.starts_with("hour") || unit.starts_with("hr") {
                    hours = n;
                    has_parts = true;
                } else if unit.starts_with("minute") || unit.starts_with("min") {
                    minutes = n;
                    has_parts = true;
                }
            }
        }
    }

    if has_parts {
        if days > 0 {
            if hours > 0 {
                format!("Up: {}d {}h", days, hours)
            } else {
                format!("Up: {} day{}", days, if days > 1 { "s" } else { "" })
            }
        } else if hours > 0 {
            if minutes > 0 {
                format!("Up: {}h {}m", hours, minutes)
            } else {
                format!("Up: {} hr{}", hours, if hours > 1 { "s" } else { "" })
            }
        } else {
            format!("Up: {} min", minutes)
        }
    } else {
        // Fallback: keep readable string with "Up: " prefix
        if lower.starts_with("up ") {
            format!("Up: {}", &clean[3..].trim())
        } else {
            format!("Up: {}", clean)
        }
    }
}

/// Parses the multi-part output of `TELEMETRY_PROBE_CMD` into structured `RemoteSystemMetrics`.
pub fn parse_remote_telemetry_output(
    raw_output: &str,
    prev_net: Option<(u64, u64, Instant)>,
) -> (RemoteSystemMetrics, Option<(u64, u64, Instant)>) {
    let mut metrics = RemoteSystemMetrics::default();

    let mut mem_total_k: Option<u64> = None;
    let mut mem_avail_k: Option<u64> = None;
    let mut total_rx = 0u64;
    let mut total_tx = 0u64;
    let mut found_net = false;

    for line in raw_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // 1. Line starting with or containing CORES:
        if trimmed.contains("CORES:") {
            let mut cores_val: Option<u32> = None;
            let mut load_val: Option<f32> = None;

            for segment in trimmed.split('|') {
                let seg = segment.trim();
                if let Some(val) = seg.strip_prefix("CORES:") {
                    if let Ok(c) = val.trim().parse::<u32>() {
                        cores_val = Some(c);
                        metrics.cores = Some(c);
                    }
                } else if let Some(val) = seg.strip_prefix("LOAD:") {
                    if let Ok(l) = val.trim().parse::<f32>() {
                        load_val = Some(l);
                    }
                } else if let Some(val) = seg.strip_prefix("UPTIME:") {
                    let u_raw = val.trim();
                    if !u_raw.is_empty() {
                        metrics.uptime = Some(format_uptime_string(u_raw));
                    }
                }
            }

            if let Some(load) = load_val {
                let cores_cnt = cores_val.unwrap_or(1) as f32;
                let pct = ((load / cores_cnt) * 100.0).round() as u32;
                metrics.cpu_pct = Some(pct.clamp(1, 99));
            }
            continue;
        }

        // 2. RAM Total and Available from /proc/meminfo
        if trimmed.starts_with("MemTotal:") {
            if let Some(v) = trimmed.split_whitespace().nth(1) {
                if let Ok(k) = v.parse::<u64>() {
                    mem_total_k = Some(k);
                }
            }
            continue;
        }
        if trimmed.starts_with("MemAvailable:") {
            if let Some(v) = trimmed.split_whitespace().nth(1) {
                if let Ok(k) = v.parse::<u64>() {
                    mem_avail_k = Some(k);
                }
            }
            continue;
        }

        // 3. Disk from df -k /
        if trimmed.contains('/') && trimmed.contains('%') {
            for token in trimmed.split_whitespace() {
                if token.ends_with('%') {
                    let num = &token[..token.len() - 1];
                    if let Ok(pct) = num.parse::<u32>() {
                        metrics.disk_pct = Some(pct.min(100));
                        break;
                    }
                }
            }
            continue;
        }

        // 4. Network interfaces from /proc/net/dev
        if let Some((iface, data)) = trimmed.split_once(':') {
            let iface = iface.trim();
            if iface != "lo"
                && !iface.starts_with("docker")
                && !iface.starts_with("br-")
                && !iface.starts_with("veth")
            {
                let fields: Vec<&str> = data.split_whitespace().collect();
                if fields.len() >= 9 {
                    if let (Ok(rx), Ok(tx)) = (fields[0].parse::<u64>(), fields[8].parse::<u64>()) {
                        total_rx += rx;
                        total_tx += tx;
                        found_net = true;
                    }
                }
            }
        }
    }

    // Compute RAM metrics
    if let Some(total_k) = mem_total_k {
        if total_k > 0 {
            let total_gb = total_k as f32 / 1024.0 / 1024.0;
            let avail_k = mem_avail_k.unwrap_or(0);
            let used_gb = (total_k.saturating_sub(avail_k)) as f32 / 1024.0 / 1024.0;
            let pct = ((used_gb / total_gb.max(0.1)) * 100.0).round() as u32;

            metrics.ram_total_gb = Some((total_gb * 10.0).round() / 10.0);
            metrics.ram_used_gb = Some((used_gb * 10.0).round() / 10.0);
            metrics.ram_pct = Some(pct.min(100));
        }
    }

    // Compute Network speed delta
    let next_net = if found_net {
        let now = Instant::now();
        metrics.net_rx_bytes = Some(total_rx);
        metrics.net_tx_bytes = Some(total_tx);

        if let Some((prev_rx, prev_tx, prev_time)) = prev_net {
            let dt = (now - prev_time).as_secs_f32().max(0.1);
            let rx_bps = (total_rx.saturating_sub(prev_rx) as f32) / dt;
            let tx_bps = (total_tx.saturating_sub(prev_tx) as f32) / dt;
            metrics.net_rate = Some(format!("▲{}  ▼{}", format_net_speed(tx_bps), format_net_speed(rx_bps)));
        }
        Some((total_rx, total_tx, now))
    } else {
        prev_net
    };

    (metrics, next_net)
}

/// Reads local root filesystem disk usage percentage using libc::statvfs (Unix only)
#[cfg(unix)]
pub fn read_local_disk_pct() -> Option<u32> {
    use std::ffi::CString;
    let path = CString::new("/").ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let res = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if res == 0 {
        let stat = unsafe { stat.assume_init() };
        let total = stat.f_blocks;
        let avail = stat.f_bavail;
        if total > 0 {
            let used = total.saturating_sub(avail);
            return Some(((used as f64 / total as f64 * 100.0).round() as u32).min(100));
        }
    }
    None
}

#[cfg(not(unix))]
pub fn read_local_disk_pct() -> Option<u32> {
    None
}

/// Reads local network byte counters from /proc/net/dev
pub fn read_local_net_bytes() -> Option<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut total_rx = 0u64;
    let mut total_tx = 0u64;
    for line in content.lines().skip(2) {
        if let Some((iface, data)) = line.split_once(':') {
            let iface = iface.trim();
            if iface == "lo"
                || iface.starts_with("docker")
                || iface.starts_with("br-")
                || iface.starts_with("veth")
            {
                continue;
            }
            let fields: Vec<&str> = data.split_whitespace().collect();
            if fields.len() >= 9 {
                if let (Ok(rx), Ok(tx)) = (fields[0].parse::<u64>(), fields[8].parse::<u64>()) {
                    total_rx += rx;
                    total_tx += tx;
                }
            }
        }
    }
    Some((total_rx, total_tx))
}

/// Collects local system telemetry metrics for the local tab
pub fn collect_local_telemetry(
    local: &mut TabTelemetry,
    last_net: &mut Option<(u64, u64)>,
    last_time: &mut Option<Instant>,
) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(h) = std::env::var("COMPUTERNAME") { local.host = h; }
        if let Ok(u) = std::env::var("USERNAME") { local.user = u; }
        local.cores = std::thread::available_parallelism().map(|n| n.get() as u32).ok();
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(h) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
            let h_clean = h.trim();
            if !h_clean.is_empty() {
                local.host = h_clean.to_uppercase();
            }
        }
        if let Ok(u) = std::env::var("USER") {
            local.user = u;
        }
        let cores_opt = std::thread::available_parallelism().map(|n| n.get() as u32).ok();
        local.cores = cores_opt;

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
                let total_gb = total as f32 / 1024.0 / 1024.0;
                let used_gb = (total.saturating_sub(avail)) as f32 / 1024.0 / 1024.0;
                local.ram_total_gb = Some((total_gb * 10.0).round() / 10.0);
                local.ram_used_gb = Some((used_gb * 10.0).round() / 10.0);
                local.ram_pct = Some(((used_gb / total_gb.max(0.1)) * 100.0).round() as u32);
            }
        }

        if let Ok(load) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(val) = load.split_whitespace().next().and_then(|v| v.parse::<f32>().ok()) {
                let cores_cnt = cores_opt.unwrap_or(1) as f32;
                let pct = ((val / cores_cnt) * 100.0).round() as u32;
                local.cpu_pct = Some(pct.clamp(1, 99));
            }
        }

        if let Ok(up) = std::fs::read_to_string("/proc/uptime") {
            if let Some(secs) = up.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()) {
                let total_s = secs as u64;
                local.uptime_secs = Some(total_s);
                let days = total_s / 86400;
                let hours = (total_s % 86400) / 3600;
                let mins = (total_s % 3600) / 60;
                if days > 0 {
                    local.uptime = Some(format!("Up: {} day{}", days, if days > 1 { "s" } else { "" }));
                } else if hours > 0 {
                    local.uptime = Some(format!("Up: {} hr{}", hours, if hours > 1 { "s" } else { "" }));
                } else {
                    local.uptime = Some(format!("Up: {} min{}", mins, if mins > 1 { "s" } else { "" }));
                }
            }
        }
        local.disk_pct = read_local_disk_pct();

        // Real Network Throughput calculation from /proc/net/dev
        if let Some((curr_rx, curr_tx)) = read_local_net_bytes() {
            let now = Instant::now();
            if let (Some((prev_rx, prev_tx)), Some(prev_t)) = (*last_net, *last_time) {
                let dt = (now - prev_t).as_secs_f32().max(0.1);
                let rx_bps = (curr_rx.saturating_sub(prev_rx) as f32) / dt;
                let tx_bps = (curr_tx.saturating_sub(prev_tx) as f32) / dt;
                local.net_rate = Some(format!("▲{}  ▼{}", format_net_speed(tx_bps), format_net_speed(rx_bps)));
            }
            *last_net = Some((curr_rx, curr_tx));
            *last_time = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_remote_telemetry_user_output() {
        let sample = r#"CORES: 8 | LOAD: 0.20 | UPTIME: up 19 hours, 11 minutes
MemTotal:       24026868 kB
MemAvailable:    8720952 kB
/dev/sda1       193G  166G   28G  86% /
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 24707833   29828    0    0    0     0          0         0 24707833   29828    0    0    0     0       0          0
  eth0: 10000000   12345    0    0    0     0          0         0  5000000    9876    0    0    0     0       0          0
"#;

        let (metrics, net_sample) = parse_remote_telemetry_output(sample, None);
        assert_eq!(metrics.cores, Some(8));
        // load 0.20 / 8 cores = 2.5% -> rounded to 3%
        assert_eq!(metrics.cpu_pct, Some(3));
        assert_eq!(metrics.uptime, Some("Up: 19h 11m".to_string()));

        // RAM: 24026868 kB total = ~22.9 GB, 8720952 kB avail = ~8.3 GB, used = 14.6 GB
        assert!(metrics.ram_total_gb.unwrap() > 22.0 && metrics.ram_total_gb.unwrap() < 24.0);
        assert!(metrics.ram_used_gb.unwrap() > 14.0 && metrics.ram_used_gb.unwrap() < 16.0);
        assert_eq!(metrics.ram_pct, Some(64));

        // Disk: 86%
        assert_eq!(metrics.disk_pct, Some(86));

        // Network: first sample records base bytes
        assert!(net_sample.is_some());
        let (rx, tx, _) = net_sample.unwrap();
        assert_eq!(rx, 10000000);
        assert_eq!(tx, 5000000);
    }

    #[test]
    fn test_parse_remote_telemetry_second_tick_network_rate() {
        let sample1 = r#"CORES: 4 | LOAD: 1.00 | UPTIME: up 1 hour, 30 minutes
MemTotal:       8000000 kB
MemAvailable:   4000000 kB
/dev/sda1       100G  50G   50G  50% /
eth0: 1000000   100    0    0    0     0          0         0   500000    100    0    0    0     0       0          0
"#;
        let (_, net1) = parse_remote_telemetry_output(sample1, None);

        // Simulate 2 seconds later with extra 204800 bytes rx (100 KB/s) and 409600 bytes tx (200 KB/s)
        let sample2 = r#"CORES: 4 | LOAD: 1.00 | UPTIME: up 1 hour, 30 minutes
MemTotal:       8000000 kB
MemAvailable:   4000000 kB
/dev/sda1       100G  50G   50G  50% /
eth0: 1204800   150    0    0    0     0          0         0   909600    150    0    0    0     0       0          0
"#;
        let (metrics2, _) = parse_remote_telemetry_output(sample2, net1);
        assert!(metrics2.net_rate.is_some());
        let rate_str = metrics2.net_rate.unwrap();
        assert!(rate_str.starts_with('▲') && rate_str.contains('▼'));
    }

    #[test]
    fn test_format_uptime_string() {
        assert_eq!(format_uptime_string("up 26 minutes"), "Up: 26 min");
        assert_eq!(format_uptime_string("up 19 hours, 11 minutes"), "Up: 19h 11m");
        assert_eq!(format_uptime_string("up 2 days, 3 hours"), "Up: 2d 3h");
        assert_eq!(format_uptime_string("up 1 day"), "Up: 1 day");
        assert_eq!(format_uptime_string("up 3 days"), "Up: 3 days");
        assert_eq!(format_uptime_string("up 1 hour"), "Up: 1 hr");
    }

    #[test]
    fn test_format_net_speed() {
        assert_eq!(format_net_speed(500.0), "0.5K");
        assert_eq!(format_net_speed(102400.0), "100.0K");
        assert_eq!(format_net_speed(2097152.0), "2.00M");
    }

    #[test]
    fn test_tab_telemetry_placeholders_and_metrics() {
        let telem = TabTelemetry::new_local();
        assert!(!telem.is_remote);
        assert!(telem.is_connected);

        let mut metrics = RemoteSystemMetrics::default();
        metrics.cores = Some(16);
        metrics.cpu_pct = Some(12);
        metrics.ram_used_gb = Some(8.5);
        metrics.ram_total_gb = Some(32.0);
        metrics.ram_pct = Some(27);
        metrics.uptime = Some("Up: 5d 2h".into());
        metrics.disk_pct = Some(42);
        metrics.net_rate = Some("▲1.2M  ▼450K".into());

        let mut remote_telem = TabTelemetry {
            host: "srv-prod".into(),
            user: "ubuntu".into(),
            cores: None,
            cpu_pct: None,
            ram_used_gb: None,
            ram_total_gb: None,
            ram_pct: None,
            uptime: None,
            uptime_secs: None,
            disk_pct: None,
            net_rate: None,
            is_remote: true,
            is_connected: true,
            last_updated: Instant::now(),
        };

        remote_telem.apply_metrics(&metrics);
        assert_eq!(remote_telem.cores, Some(16));
        assert_eq!(remote_telem.cpu_pct, Some(12));
        assert_eq!(remote_telem.ram_used_gb, Some(8.5));
        assert_eq!(remote_telem.ram_total_gb, Some(32.0));
        assert_eq!(remote_telem.ram_pct, Some(27));
        assert_eq!(remote_telem.uptime, Some("Up: 5d 2h".into()));
        assert_eq!(remote_telem.disk_pct, Some(42));
        assert_eq!(remote_telem.net_rate, Some("▲1.2M  ▼450K".into()));
    }
}
