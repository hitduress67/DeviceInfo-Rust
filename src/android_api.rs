use android_activity::AndroidApp;
use jni::objects::{JObject, JString, JValue};
use jni::JNIEnv;
use ndk_sys::ANativeActivity;
use std::io::Read;
use std::net::TcpStream;

pub fn collect_system_info(app: &AndroidApp) -> Vec<String> {
    let mut lines = Vec::new();
    
    let vm = app.vm();
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(_) => {
            lines.push("JNI Error: attach failed".to_string());
            return lines;
        }
    };

    // Get context via ActivityThread (works without needing NativeActivity reference)
    let context = get_context_via_activity_thread(&mut env);
    let ctx_ref = context.as_ref().map(|c| c as *const JObject);
    
    lines.push("── Device ──".to_string());
    add_field(&mut lines, "Model",     build_string(&mut env, "MODEL"));
    add_field(&mut lines, "Manufacturer", build_string(&mut env, "MANUFACTURER"));
    add_field(&mut lines, "Android",  get_android_version(&mut env));
    add_field(&mut lines, "SDK",      build_int_string(&mut env, "VERSION.SDK_INT"));
    add_field(&mut lines, "Build",    build_string(&mut env, "DISPLAY"));
    
    lines.push("".to_string());
    lines.push("── Performance ──".to_string());
    add_field(&mut lines, "CPU",        get_cpu_info());
    add_field(&mut lines, "Cores",      get_cpu_cores());
    add_field(&mut lines, "RAM",        get_ram_info2(&mut env));
    add_field(&mut lines, "RAM Used",   get_ram_used(&mut env));
    
    lines.push("".to_string());
    lines.push("── Storage ──".to_string());
    add_field(&mut lines, "Internal",   get_storage_info2());
    
    lines.push("".to_string());
    lines.push("── Battery ──".to_string());
    add_field(&mut lines, "Level",      get_battery_level2(&mut env));
    add_field(&mut lines, "Temp",       get_battery_temp2(&mut env));
    add_field(&mut lines, "Status",     get_charging_status2(&mut env));
    
    lines.push("".to_string());
    lines.push("── Display ──".to_string());
    add_field(&mut lines, "Resolution", get_screen_res2(&mut env));
    add_field(&mut lines, "Density",    get_screen_density2(&mut env));
    
    lines.push("".to_string());
    lines.push("── Network ──".to_string());
    add_field(&mut lines, "Type",       get_network_type2(&mut env));
    add_field(&mut lines, "Status",     get_connection_status2(&mut env));
    add_field(&mut lines, "Local IP",   get_local_ip2());
    add_field(&mut lines, "Public IP",  get_public_ip2());
    add_field(&mut lines, "Capable",    get_cellular_capability2(&mut env));
    
    lines.push("".to_string());
    lines.push("── System ──".to_string());
    add_field(&mut lines, "Sensors",    get_sensor_count2(&mut env));
    add_field(&mut lines, "Uptime",     get_uptime2(&mut env));
    
    lines.push("".to_string());
    lines.push("  Rust Native (v0.1.0)".to_string());
    
    lines
}

fn add_field(lines: &mut Vec<String>, label: &str, value: String) {
    lines.push(format!("  {}: {}", label, value));
}

// ── Context via ActivityThread (works without requiring NativeActivity) ──
fn get_context_via_activity_thread(env: &mut JNIEnv) -> Option<JObject> {
    let at_cls = env.find_class("android/app/ActivityThread").ok()?;
    let cur_method = env.get_static_method_id(at_cls, "currentActivityThread", "()Landroid/app/ActivityThread;").ok()?;
    let at_obj = env.call_static_method(at_cls, cur_method, &[]).ok()?;
    let at = at_obj.l()?;
    let app_method = env.get_method_id(at_cls, "getApplication", "()Landroid/app/Application;").ok()?;
    let app_obj = env.call_method(&at, app_method, &[]).ok()?;
    app_obj.l()
}

// ── Build Fields ──
fn build_string(env: &mut JNIEnv, field: &str) -> String {
    call_build_field(env, field, "Ljava/lang/String;")
        .and_then(|v| v.l())
        .and_then(|o| env.get_string(&JString::from(o)).ok())
        .map(|s| s.into())
        .unwrap_or_else(|| "N/A".to_string())
}

fn build_int_string(env: &mut JNIEnv, field: &str) -> String {
    call_build_field(env, field, "I")
        .and_then(|v| {
            if let JValue::Int(n) = v { Some(n.to_string()) } else { None }
        })
        .unwrap_or_else(|| "N/A".to_string())
}

fn call_build_field(env: &mut JNIEnv, field_path: &str, sig: &str) -> Option<JValue> {
    let parts: Vec<&str> = field_path.split('.').collect();
    if parts.is_empty() { return None; }
    // Build class name for inner classes: android/os/Build$VERSION for VERSION.RELEASE
    let class_name = if parts.len() > 1 {
        let inner = parts[..parts.len()-1].join("$");
        format!("android/os/{}", inner.replace(".", "$"))
    } else {
        "android/os/Build".to_string()
    };
    let field_name = parts[parts.len()-1];
    let cls = env.find_class(&class_name).ok()?;
    let fid = env.get_static_field_id(cls, field_name, sig).ok()?;
    if sig == "I" {
        env.get_static_field(cls, fid, JValue::Int).ok()
    } else {
        env.get_static_field(cls, fid, JValue::Object).ok()
    }
}

fn get_android_version(env: &mut JNIEnv) -> String {
    build_string(env, "VERSION.RELEASE")
}

// ── CPU Info (pure Rust, no JNI) ──
fn get_cpu_info() -> String {
    let data = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    for line in data.lines() {
        if line.starts_with("Hardware") || line.starts_with("Processor") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 { return parts[1].trim().to_string(); }
        }
    }
    // Also try /proc/device-tree/model (found on some devices)
    if let Ok(m) = std::fs::read_to_string("/proc/device-tree/model") {
        let t = m.trim_matches('\0').trim();
        if !t.is_empty() { return t.to_string(); }
    }
    "Unknown".to_string()
}

fn get_cpu_cores() -> String {
    let count = std::fs::read_dir("/sys/devices/system/cpu")
        .map(|e| e.filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy();
                n.starts_with("cpu") && n.len() > 3 && n[3..].chars().all(|c| c.is_ascii_digit())
            })
            .count())
        .unwrap_or(0);
    format!("{} cores", count)
}

// ── RAM via /proc/meminfo (pure Rust, no JNI needed) ──
fn get_ram_info2() -> String {
    let data = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
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
    if total == 0 { return "N/A".to_string(); }
    format!("{} / {}", fmt_kb(total), fmt_kb(avail))
}

fn get_ram_used() -> String {
    let data = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
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
    if total == 0 { return "N/A".to_string(); }
    let used = total.saturating_sub(avail);
    let pct = used * 100 / total;
    format!("{}% used", pct)
}

fn fmt_kb(kb: u64) -> String {
    let bytes = kb * 1024;
    if bytes < 1024*1024 { format!("{} MB", bytes / 1024 / 1024) }
    else { format!("{:.1} GB", bytes as f64 / (1024.0*1024.0*1024.0)) }
}

// ── Storage via /proc/stat (pure Rust) ──
fn get_storage_info2() -> String {
    match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(data) => {
            for line in data.lines() {
                // Find /data mount point
                if line.contains(" /data ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 5 {
                        let dev = parts[3].trim();
                        if !dev.is_empty() && !dev.starts_with("/dev/loop") {
                            return format!("Mount: {}", dev);
                        }
                    }
                }
            }
            "See Settings".to_string()
        }
        Err(_) => "See Settings".to_string(),
    }
}

// ── Battery via JNI ──
fn get_battery_level2(env: &mut JNIEnv) -> String {
    let intent = get_battery_intent(env);
    match intent {
        Some(i) => {
            let level = env.get_field(&i, "level", "I").ok()
                .and_then(|v| if let JValue::Int(n) = v { Some(n) } else { None }).unwrap_or(0);
            let scale = env.get_field(&i, "scale", "I").ok()
                .and_then(|v| if let JValue::Int(n) = v { Some(n) } else { None }).unwrap_or(100);
            if scale == 0 { return "N/A".to_string(); }
            format!("{}%", level * 100 / scale)
        }
        None => "N/A".to_string(),
    }
}

fn get_battery_temp2(env: &mut JNIEnv) -> String {
    let intent = get_battery_intent(env);
    match intent {
        Some(i) => {
            let temp = env.get_field(&i, "temperature", "I").ok()
                .and_then(|v| if let JValue::Int(n) = v { Some(n) } else { None }).unwrap_or(0);
            format!("{:.1}°C", temp as f64 / 10.0)
        }
        None => "N/A".to_string(),
    }
}

fn get_charging_status2(env: &mut JNIEnv) -> String {
    let intent = get_battery_intent(env);
    match intent {
        Some(i) => {
            let status = env.get_field(&i, "status", "I").ok()
                .and_then(|v| if let JValue::Int(n) = v { Some(n) } else { None }).unwrap_or(-1);
            match status {
                2 => "Charging".to_string(),
                3 => "Discharging".to_string(),
                4 => "Not charging".to_string(),
                5 => "Full".to_string(),
                _ => format!("{}", status),
            }
        }
        None => "N/A".to_string(),
    }
}

fn get_battery_intent(env: &mut JNIEnv) -> Option<JObject> {
    let ctx = get_context_via_activity_thread(env)?;
    // Call context.registerReceiver(null, IntentFilter(BatteryManager.ACTION_BATTERY_CHANGED))
    let intent_cls = env.find_class("android/content/IntentFilter").ok()?;
    let if_ctor = env.get_method_id(intent_cls, "<init>", "(Ljava/lang/String;)V").ok()?;
    let battery_action = env.new_string("android.intent.action.BATTERY_CHANGED").ok()?;
    let filter = env.new_object(intent_cls, if_ctor, &[JValue::Object(&battery_action.into())]).ok()?;
    let ctx_cls = env.get_object_class(&ctx).ok()?;
    let reg_method = env.get_method_id(ctx_cls, "registerReceiver", "(Landroid/content/BroadcastReceiver;Landroid/content/IntentFilter;)Landroid/content/Intent;").ok()?;
    let intent = env.call_method(&ctx, reg_method, &[JValue::Object(&JObject::null()), JValue::Object(&filter)]).ok()?;
    intent.l()
}

fn get_network_type2(env: &mut JNIEnv) -> String {
    "N/A".to_string()
}

fn get_connection_status2(env: &mut JNIEnv) -> String {
    "N/A".to_string()
}

fn get_cellular_capability2(env: &mut JNIEnv) -> String {
    "N/A".to_string()
}

fn get_sensor_count2(env: &mut JNIEnv) -> String {
    "N/A".to_string()
}

fn get_screen_res2(env: &mut JNIEnv) -> String {
    "N/A".to_string()
}

fn get_screen_density2(env: &mut JNIEnv) -> String {
    "N/A".to_string()
}

// ── Local IP via std::net ──
fn get_local_ip2() -> String {
    // Read from network interfaces via /proc (pure Rust, stable API)
    if let Ok(data) = std::fs::read_to_string("/proc/net/fib_trie") {
        // Parse for local IPs
        for line in data.lines() {
            let line = line.trim();
            if line.starts_with("+--") || line.starts_with("|--") {
                continue;
            }
            if line.contains("LOCAL") {
                if let Some(ip_line) = data.lines()
                    .filter(|l| l.trim().contains('.'))
                    .next()
                {
                    let ip = ip_line.trim().split_whitespace().next().unwrap_or("");
                    if !ip.is_empty() && ip != "127.0.0.1" {
                        return ip.to_string();
                    }
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

// ── Public IP via raw TCP (port 80, plain HTTP) ──
fn get_public_ip2() -> String {
    for host in &["api.ipify.org:80", "icanhazip.com:80"] {
        if let Ok(mut stream) = TcpStream::connect_timeout(
            &host.parse().unwrap(),
            std::time::Duration::from_secs(3),
        ) {
            let req = format!("GET / HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
                host.trim_end_matches(":80"));
            let _ = std::io::Write::write_all(&mut stream, req.as_bytes());
            let mut response = String::new();
            stream.read_to_string(&mut response).ok();
            for line in response.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty()
                    && !trimmed.starts_with("HTTP/")
                    && !trimmed.starts_with("<!")
                    && !trimmed.starts_with("Date:")
                    && !trimmed.starts_with("Server:")
                    && !trimmed.starts_with("Content-")
                    && !trimmed.starts_with("Connection:")
                    && !trimmed.starts_with("X-")
                {
                    return trimmed.to_string();
                }
            }
        }
    }
    "Unavailable".to_string()
}

// ── Uptime via /proc/uptime (pure Rust, no JNI) ──
fn get_uptime2(env: &mut JNIEnv) -> String {
    let data = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    let secs = data.split_whitespace().next()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| s as u64)
        .unwrap_or(0);
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!("{}d {}h {}m", days, hours, mins)
}

// ── Helper ──
fn fmt_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b < 1024.0 { return format!("{} B", bytes); }
    let e = (b.ln() / 1024_f64.ln()).floor() as i32;
    let prefixes = ["K", "M", "G", "T", "P"];
    let i = (e - 1).min(4) as usize;
    format!("{:.1}{}B", b / 1024_f64.powi(e), prefixes[i])
}
