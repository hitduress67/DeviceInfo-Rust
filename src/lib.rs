use log::info;
use std::sync::Mutex;

mod sysinfo;

static RENDER_DATA: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn ANativeActivity_onCreate(
    _activity: *mut std::ffi::c_void,
    _saved_state: *mut std::ffi::c_void,
    _saved_state_size: usize,
) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SysInfo"),
    );

    info!("SysInfo Dashboard Rust v0.2.0 starting");

    // Collect info once
    let lines = sysinfo::collect_system_info();
    if let Ok(mut data) = RENDER_DATA.lock() {
        *data = lines.clone();
    }
    for line in &lines {
        info!("{}", line);
    }

    // Set up activity callbacks
    let activity = activity as *mut ndk_sys::ANativeActivity;
    unsafe {
        let native_activity = ndk::native_activity::NativeActivity::new(activity);
        
        // Set input and event callbacks
        (*activity).callbacks = Box::into_raw(Box::new(ndk_sys::ANativeActivityCallbacks {
            on_start: Some(on_start),
            on_resume: Some(on_resume),
            on_save_instance_state: None,
            on_pause: Some(on_pause),
            on_stop: Some(on_stop),
            on_destroy: Some(on_destroy),
            on_window_focus_changed: None,
            on_native_window_created: Some(on_window_created),
            on_native_window_resized: Some(on_window_resized),
            on_native_window_redraw_needed: Some(on_window_redraw),
            on_native_window_destroyed: Some(on_window_destroyed),
            on_input_queue_created: None,
            on_input_queue_destroyed: None,
            on_content_rect_changed: None,
            on_configuration_changed: None,
            on_low_memory: None,
        }));
    }

    info!("Callbacks registered");
}

unsafe extern "C" fn on_start(activity: *mut ndk_sys::ANativeActivity) {
    info!("on_start");
}

unsafe extern "C" fn on_resume(activity: *mut ndk_sys::ANativeActivity) {
    info!("on_resume");
}

unsafe extern "C" fn on_pause(activity: *mut ndk_sys::ANativeActivity) {
    info!("on_pause");
}

unsafe extern "C" fn on_stop(activity: *mut ndk_sys::ANativeActivity) {
    info!("on_stop");
}

unsafe extern "C" fn on_destroy(activity: *mut ndk_sys::ANativeActivity) {
    info!("on_destroy");
    // Free the callbacks
    if !(*activity).callbacks.is_null() {
        let _ = Box::from_raw((*activity).callbacks);
        (*activity).callbacks = std::ptr::null_mut();
    }
}

unsafe extern "C" fn on_window_created(activity: *mut ndk_sys::ANativeActivity, window: *mut ndk_sys::ANativeWindow) {
    info!("on_window_created");
    render_frame(window);
}

unsafe extern "C" fn on_window_resized(activity: *mut ndk_sys::ANativeActivity, window: *mut ndk_sys::ANativeWindow) {
    info!("on_window_resized");
    render_frame(window);
}

unsafe extern "C" fn on_window_redraw(activity: *mut ndk_sys::ANativeActivity, window: *mut ndk_sys::ANativeWindow) {
    render_frame(window);
}

unsafe extern "C" fn on_window_destroyed(activity: *mut ndk_sys::ANativeActivity, window: *mut ndk_sys::ANativeWindow) {
    info!("on_window_destroyed");
}

unsafe fn render_frame(window: *mut ndk_sys::ANativeWindow) {
    let lines = RENDER_DATA.lock().ok().map(|d| d.clone()).unwrap_or_default();

    // Lock the window buffer
    let mut buffer: std::mem::MaybeUninit<ndk_sys::ANativeWindow_Buffer> = std::mem::MaybeUninit::uninit();
    let result = ndk_sys::ANativeWindow_lock(
        window,
        buffer.as_mut_ptr(),
        std::ptr::null_mut(),
    );

    if result != 0 {
        info!("ANativeWindow_lock failed: {}", result);
        return;
    }

    let buf = buffer.assume_init();
    let width = buf.width as usize;
    let height = buf.height as usize;
    let stride = buf.stride as usize;
    let pixels = buf.bits as *mut u32;

    // Fill background
    let bg_color = 0xFF121212u32;
    let fg_color = 0xFFFFFFFFu32;
    let accent = 0xFF90CAF9u32;
    let dim = 0xFF80FFFFFFu32;

    for y in 0..height {
        for x in 0..width {
            *pixels.add(y * stride + x) = bg_color;
        }
    }

    // Simple text renderer using 8x8 bitmap characters rendered as blocks
    // Each character is ~8x8 pixels at scale 2 = 16x16 blocks
    let scale = 2usize;
    let char_w = 6 * scale;
    let char_h = 8 * scale;
    let mut row = 10;
    let mut col = 10;

    // Title bar
    let title = "SysInfo Dashboard v0.2.0";
    let title_color = accent;
    draw_text(pixels, width, height, stride, title, col, row, title_color, scale);
    row += char_h + 8;

    let line_h = char_h + 4;

    for line in &lines {
        if row + line_h >= height {
            break;
        }
        if line.is_empty() {
            row += char_h / 2;
            continue;
        }
        let color = if line.starts_with("--") { accent }
                   else if line.starts_with("  ") { fg_color }
                   else { dim };
        draw_text(pixels, width, height, stride, line, col, row, color, scale);
        row += line_h;
    }

    // Footer
    let footer = "Pure Rust Native (logcat + render)";
    let fw = footer.len() * char_w;
    if fw < width {
        draw_text(pixels, width, height, stride, footer,
            width - fw - 10, height - char_h - 10, 0x40FFFFFF, scale);
    }

    ndk_sys::ANativeWindow_unlockAndPost(window);
}

fn draw_text(pixels: *mut u32, width: usize, height: usize, stride: usize,
             text: &str, x: usize, y: usize, color: u32, scale: usize) {
    let mut cx = x;
    let cy = y;

    for ch in text.chars() {
        if ch == '\n' { continue; }
        if ch < ' ' || ch > '~' {
            if ch == '─' || ch == '—' || ch == '–' || ch == '·' || ch == '↕' || ch == '✓' {
                // Draw a simple dash or bullet
                let bx = cx + scale;
                let by = cy + 3 * scale;
                let bw = 4 * scale;
                let bh = 2 * scale;
                for dy in 0..bh {
                    for dx in 0..bw {
                        let px = bx + dx;
                        let py = by + dy;
                        if px < width && py < height {
                            unsafe { *pixels.add(py * stride + px) = color; }
                        }
                    }
                }
            }
            cx += 6 * scale;
            continue;
        }

        // Draw a simple block character (just a filled rectangle for each letter)
        // More sophisticated rendering would use a bitmap font
        let bw = 5 * scale;
        let bh = 7 * scale;

        // Simple block: fill the character area with the color
        let left_margin = scale;
        for dy in 0..bh {
            for dx in 0..bw {
                let px = cx + left_margin + dx;
                let py = cy + dy;
                if px < width && py < height {
                    unsafe { *pixels.add(py * stride + px) = color; }
                }
            }
        }

        // Draw a tiny gap between characters (vertical line of background color)
        let gap_x = cx + left_margin + bw;
        for dy in 0..bh {
            if gap_x < width && cy + dy < height {
                unsafe { *pixels.add((cy + dy) * stride + gap_x) = 0xFF121212u32; }
            }
        }

        cx += 7 * scale;
    }
}
