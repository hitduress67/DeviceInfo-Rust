use android_activity::AndroidApp;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jint, jlong, jstring};
use jni::JNIEnv;
use std::ffi::CStr;
use std::net::TcpStream;
use std::io::Read;

/// Collect all system info into a Vec of (section, label, value) tuples rendered as lines
pub fn collect_system_info(app: &AndroidApp) -> Vec<String> {
    let mut lines = Vec::new();
    
    let vm = app.vm();
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(e) => {
            lines.push("JNI Error: failed to attach".to_string());
            return lines;
        }
    };
    
    // ── Device Section ──
    lines.push("── Device ──".to_string());
    add_field(&mut lines, "Model", get_build_field(&mut env, "MODEL"));
    add_field(&mut lines, "Manufacturer", get_build_field(&mut env, "MANUFACTURER"));
    add_field(&mut lines, "Brand", get_build_field(&mut env, "BRAND"));
    add_field(&mut lines, "Android", get_build_field(&mut env, "VERSION.RELEASE"));
    add_field(&mut lines, "SDK", get_build_field(&mut env, "VERSION.SDK_INT"));
    add_field(&mut lines, "Build", get_build_field(&mut env, "DISPLAY"));
    
    // ── Performance Section ──
    lines.push("".to_string());
    lines.push("── Performance ──".to_string());
    add_field(&mut lines, "CPU", get_cpu_info());
    add_field(&mut lines, "Cores", get_cpu_cores());
    add_field(&mut lines, "RAM", get_ram_info(&mut env, &app));
    
    // ── Storage Section ──
    lines.push("".to_string());
    lines.push("── Storage ──".to_string());
    add_field(&mut lines, "Internal", get_storage_info(&mut env));
    
    // ── Battery Section ──
    lines.push("".to_string());
    lines.push("── Battery ──".to_string());
    add_field(&mut lines, "Level", get_battery_level(&mut env, &app));
    add_field(&mut lines, "Temp", get_battery_temp(&mut env, &app));
    add_field(&mut lines, "Status", get_charging_status(&mut env, &app));
    
    // ── Display Section ──
    lines.push("".to_string());
    lines.push("── Display ──".to_string());
    add_field(&mut lines, "Resolution", get_screen_res(&mut env, &app));
    add_field(&mut lines, "Density", get_screen_density(&mut env, &app));
    
    // ── Network Section ──
    lines.push("".to_string());
    lines.push("── Network ──".to_string());
    add_field(&mut lines, "Type", get_network_type(&mut env, &app));
    add_field(&mut lines, "Status", get_connection_status(&mut env, &app));
    add_field(&mut lines, "Local IP", get_local_ip(&mut env, &app));
    add_field(&mut lines, "Public IP", get_public_ip());
    add_field(&mut lines, "Capable", get_cellular_capability(&mut env, &app));
    
    // ── System Section ──
    lines.push("".to_string());
    lines.push("── System ──".to_string());
    add_field(&mut lines, "Sensors", get_sensor_count(&mut env, &app));
    add_field(&mut lines, "Uptime", get_uptime());
    
    lines.push("".to_string());
    lines.push("  Rust Native (v0.1.0)".to_string());
    
    lines
}

fn add_field(lines: &mut Vec<String>, label: &str, value: String) {
    lines.push(format!("  {}: {}", label, value));
}

// ── Build Fields ──
fn get_build_field(env: &mut JNIEnv, field_path: &str) -> String {
    // We need to traverse nested static fields
    let parts: Vec<&str> = field_path.split('.').collect();
    
    if parts.is_empty() {
        return "N/A".to_string();
    }
    
    let mut current_class = "android/os/Build";
    let mut current_obj: Option<JObject> = None;
    
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        match env.find_class(current_class) {
            Ok(cls) => {
                if is_last {
                    // Try String field first
                    let jfield_id = match env.get_static_field_id(cls, part, "Ljava/lang/String;") {
                        Ok(fid) => {
                            match env.get_static_field(cls, fid, JValue::Object) {
                                Ok(val) => {
                                    match val.l() {
                                        Some(obj) => {
                                            let jstr = JString::from(obj);
                                            match env.get_string(&jstr) {
                                                Ok(s) => return s.into(),
                                                Err(_) => return "N/A".to_string(),
                                            }
                                        }
                                        None => return "N/A".to_string(),
                                    }
                                }
                                Err(_) => return "N/A".to_string(),
                            }
                        }
                        Err(_) => {
                            // Try int field
                            match env.get_static_field_id(cls, part, "I") {
                                Ok(fid) => {
                                    match env.get_static_field(cls, fid, JValue::Int) {
                                        Ok(val) => {
                                            if let JValue::Int(n) = val {
                                                return n.to_string();
                                            }
                                            return "N/A".to_string();
                                        }
                                        Err(_) => return "N/A".to_string(),
                                    }
                                }
                                Err(_) => return "N/A".to_string(),
                            }
                        }
                    };
                }
            }
            Err(_) => return "N/A".to_string(),
        }
    }
    
    "N/A".to_string()
}

// ── CPU Info ──
fn get_cpu_info() -> String {
    let data = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    for line in data.lines() {
        if line.starts_with("Hardware") || line.starts_with("Processor") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                return parts[1].trim().to_string();
            }
        }
    }
    // Fallback: try ro.board.platform or similar from /proc
    if let Ok(hw) = std::fs::read_to_string("/proc/self/cmdline") {
        let trimmed = hw.trim_matches('\0');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "Unknown".to_string()
}

fn get_cpu_cores() -> String {
    let count = std::fs::read_dir("/sys/devices/system/cpu")
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("cpu"))
                .filter(|e| e.file_name().to_string_lossy().len() > 3)
                .count()
        })
        .unwrap_or(0);
    format!("{} cores", count)
}

// ── RAM ──
fn get_ram_info(env: &mut JNIEnv, app: &AndroidApp) -> String {
    match env.find_class("android/app/ActivityManager") {
        Ok(cls) => {
            let fid = match env.get_method_id(cls, "<init>", "(Landroid/content/Context;)V") {
                Ok(f) => f,
                Err(_) => return "N/A".to_string(),
            };
            
            // Get context
            match get_context(env, app) {
                Some(ctx) => {
                    let mgr = env.new_object(cls, fid, &[JValue::Object(&ctx)]);
                    match mgr {
                        Ok(am) => {
                            let mem_fid = match env.get_method_id(cls, "getMemoryInfo", "()Landroid/app/ActivityManager$MemoryInfo;") {
                                Ok(f) => f,
                                Err(_) => return "N/A".to_string(),
                            };
                            match env.call_method(&am, mem_fid, &[]) {
                                Ok(mem_info) => {
                                    if let JValue::Object(mi_obj) = mem_info {
                                        if let Some(mi) = mi_obj {
                                            let total_fid = match env.get_field_id(&env.find_class("android/app/ActivityManager$MemoryInfo").unwrap(), "totalMem", "J") {
                                                Ok(f) => f,
                                                Err(_) => return "N/A".to_string(),
                                            };
                                            let avail_fid = match env.get_field_id(&env.find_class("android/app/ActivityManager$MemoryInfo").unwrap(), "availMem", "J") {
                                                Ok(f) => f,
                                                Err(_) => return "N/A".to_string(),
                                            };
                                            let total: i64 = env.get_field(&mi, total_fid, JValue::Long(0)).map(|v| if let JValue::Long(n) = v { n } else { 0 }).unwrap_or(0);
                                            let avail: i64 = env.get_field(&mi, avail_fid, JValue::Long(0)).map(|v| if let JValue::Long(n) = v { n } else { 0 }).unwrap_or(0);
                                            let used = total - avail;
                                            return format!("{} GB / {} GB", format_bytes(total), format_bytes(total));
                                        }
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
                None => {}
            }
            "N/A".to_string()
        }
        Err(_) => "N/A".to_string(),
    }
}

fn get_context<'a>(env: &mut JNIEnv<'a>, app: &AndroidApp) -> Option<JObject<'a>> {
    // Try to get the context from the NativeActivity
    let cls = env.find_class("android/app/NativeActivity").ok()?;
    let activity = app.native_activity()?;
    let ctx_fid = env.get_field_id(cls, "java_activity", "Landroid/app/Activity;").ok()?;
    let ctx = env.get_field(activity, ctx_fid, JValue::Object).ok()?;
    if let JValue::Object(obj) = ctx {
        obj
    } else {
        None
    }
}

// ── Storage ──
fn get_storage_info(env: &mut JNIEnv) -> String {
    "See device settings".to_string()
}

// ── Battery ──
fn get_battery_level(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

fn get_battery_temp(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

fn get_charging_status(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

// ── Display ──
fn get_screen_res(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

fn get_screen_density(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

// ── Network ──
fn get_network_type(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

fn get_connection_status(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

fn get_local_ip(env: &mut JNIEnv, app: &AndroidApp) -> String {
    // Try reading from network interfaces (pure Rust, no JNI needed)
    for iface in std::net::NetworkInterface::list() {
        if iface.is_ok() {
            let iface = iface.unwrap();
            if iface.is_loopback() || !iface.is_up() { continue; }
            for addr in iface.addresses() {
                if let std::net::IpAddr::V4(v4) = addr.ip() {
                    return v4.to_string();
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

fn get_public_ip() -> String {
    match TcpStream::connect_timeout(
        &"api.ipify.org:80".parse().unwrap(),
        std::time::Duration::from_secs(3),
    ) {
        Ok(mut stream) => {
            let req = "GET / HTTP/1.0\r\nHost: api.ipify.org\r\nConnection: close\r\n\r\n";
            let _ = std::io::Write::write_all(&mut stream, req.as_bytes());
            let mut response = String::new();
            stream.read_to_string(&mut response).ok();
            for line in response.lines() {
                if !line.starts_with("HTTP") && !line.starts_with("<!") && !line.is_empty() {
                    return line.trim().to_string();
                }
            }
            "Unavailable".to_string()
        }
        Err(_) => {
            // Fallback to icanhazip.com
            match TcpStream::connect_timeout(
                &"icanhazip.com:80".parse().unwrap(),
                std::time::Duration::from_secs(3),
            ) {
                Ok(mut stream) => {
                    let req = "GET / HTTP/1.0\r\nHost: icanhazip.com\r\nConnection: close\r\n\r\n";
                    let _ = std::io::Write::write_all(&mut stream, req.as_bytes());
                    let mut response = String::new();
                    stream.read_to_string(&mut response).ok();
                    for line in response.lines() {
                        if !line.starts_with("HTTP") && !line.is_empty() {
                            return line.trim().to_string();
                        }
                    }
                    "Unavailable".to_string()
                }
                Err(_) => "Unavailable".to_string(),
            }
        }
    }
}

fn get_cellular_capability(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

// ── Sensors ──
fn get_sensor_count(env: &mut JNIEnv, app: &AndroidApp) -> String {
    "N/A".to_string()
}

// ── Uptime ──
fn get_uptime() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    format!("{}d {}h {}m", days, hours, mins)
}

// ── Helpers ──
fn format_bytes(bytes: i64) -> String {
    let bytes = bytes as f64;
    if bytes < 1024.0 { return format!("{} B", bytes as i64); }
    let exp = (bytes.ln() / 1024_f64.ln()).floor() as i32;
    let prefixes = ["K", "M", "G", "T", "P"];
    let idx = (exp - 1).min(4) as usize;
    format!("{:.1}{}B", bytes / 1024_f64.powi(exp), prefixes[idx])
}
