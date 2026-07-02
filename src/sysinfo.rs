use std::fs;
use std::io::Read;
use std::net::TcpStream;

pub fn collect_system_info() -> Vec<String> {
    let mut lines = Vec::new();

    lines.push("── Device ──".to_string());
    add(&mut lines, "Model",     read_first_match("/proc/cpuinfo", &["Hardware", "Processor"]));
    add(&mut lines, "CPU",       read_cpu_model());
    add(&mut lines, "Cores",     count_cpu_cores());
    add(&mut lines, "Arch",      std::env::consts::ARCH.to_string());

    lines.push("".to_string());
    lines.push("── Performance ──".to_string());
    let (total_kb, avail_kb) = read_meminfo();
    if total_kb > 0 {
        add(&mut lines, "RAM",  format!("{} / {}", fmt_kb(total_kb), fmt_kb(avail_kb)));
        let used = total_kb.saturating_sub(avail_kb);
        add(&mut lines, "Used", format!("{}%", used * 100 / total_kb));
    }

    lines.push("".to_string());
    lines.push("── Storage ──".to_string());
    add(&mut lines, "Internal", read_storage());

    lines.push("".to_string());
    lines.push("── Network ──".to_string());
    add(&mut lines, "Local IP",  read_local_ip());
    add(&mut lines, "Public IP", read_public_ip());

    lines.push("".to_string());
    lines.push("── System ──".to_string());
    add(&mut lines, "Uptime",    read_uptime());

    lines.push("".to_string());
    lines.push("  Rust Native (v0.1.0)".to_string());

    lines
}

fn add(lines: &mut Vec<String>, label: &str, value: String) {
    lines.push(format!("  {}: {}", label, value));
}

fn read_first_match(path: &str, prefixes: &[&str]) -> String {
    let data = fs::read_to_string(path).unwrap_or_default();
    for line in data.lines() {
        for prefix in prefixes {
            if line.trim().starts_with(prefix) {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    return parts[1].trim().to_string();
                }
            }
        }
    }
    "Unknown".to_string()
}

fn read_cpu_model() -> String {
    // Try /proc/cpuinfo for model name
    let data = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    for line in data.lines() {
        if line.contains("model name") || line.contains("Hardware") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                return parts[1].trim().to_string();
            }
        }
    }
    // Try /proc/device-tree/model
    if let Ok(m) = fs::read_to_string("/proc/device-tree/model") {
        let t = m.trim_matches('\0').trim();
        if !t.is_empty() { return t.to_string(); }
    }
    "Unknown".to_string()
}

fn count_cpu_cores() -> String {
    let count = fs::read_dir("/sys/devices/system/cpu")
        .map(|e| e.filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy();
                n.starts_with("cpu") && n.len() > 3 && n[3..].chars().all(|c| c.is_ascii_digit())
            })
            .count())
        .unwrap_or(0);
    format!("{}", count)
}

fn read_meminfo() -> (u64, u64) {
    let data = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0u64;
    let mut avail = 0u64;
    for line in data.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(n) = line.split_whitespace().nth(1) { total = n.parse().unwrap_or(0); }
        }
        if line.starts_with("MemAvailable:") {
            if let Some(n) = line.split_whitespace().nth(1) { avail = n.parse().unwrap_or(0); }
        }
    }
    (total, avail)
}

fn fmt_kb(kb: u64) -> String {
    if kb < 1024 {
        format!("{} MB", kb / 1024 * 1024 / 1024 / 1024)
    } else {
        format!("{:.1} GB", kb as f64 / (1024.0 * 1024.0))
    }
}

fn read_storage() -> String {
    if let Ok(data) = fs::read_to_string("/proc/self/mountinfo") {
        for line in data.lines() {
            if line.contains(" /data ") && !line.contains("/loop") {
                return "Available".to_string();
            }
        }
    }
    "See Settings".to_string()
}

fn read_local_ip() -> String {
    // Read from /proc/net/fib_trie for local IPs
    let data = fs::read_to_string("/proc/net/fib_trie").unwrap_or_default();
    let mut ips = Vec::new();
    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.len() > 7 && trimmed.chars().filter(|&c| c == '.').count() == 3 {
            let ip = trimmed.split_whitespace().next().unwrap_or("");
            if !ip.is_empty() && ip != "127.0.0.1" {
                ips.push(ip.to_string());
            }
        }
    }
    if !ips.is_empty() {
        return ips[0].clone();
    }
    "127.0.0.1".to_string()
}

fn read_public_ip() -> String {
    for host in &["api.ipify.org:80", "icanhazip.com:80"] {
        let addr = match host.parse() {
            Ok(a) => a,
            Err(_) => continue,
        };
        if let Ok(mut stream) = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)) {
            let req = format!("GET / HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
                host.trim_end_matches(":80"));
            let _ = std::io::Write::write_all(&mut stream, req.as_bytes());
            let mut response = String::new();
            if stream.read_to_string(&mut response).is_ok() {
                for line in response.lines() {
                    let t = line.trim();
                    if !t.is_empty()
                        && !t.starts_with("HTTP/")
                        && !t.starts_with("<!")
                        && !t.starts_with("Date:")
                        && !t.starts_with("Server:")
                        && !t.starts_with("Content-")
                        && !t.starts_with("Connection:")
                    {
                        return t.to_string();
                    }
                }
            }
        }
    }
    "Unavailable".to_string()
}

fn read_uptime() -> String {
    let data = fs::read_to_string("/proc/uptime").unwrap_or_default();
    let secs = data.split_whitespace().next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| s as u64)
        .unwrap_or(0);
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!("{}d {}h {}m", days, hours, mins)
}
