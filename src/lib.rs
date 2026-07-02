use log::info;
use ndk::native_activity::NativeActivity;
use ndk::native_window::NativeWindow;

mod sysinfo;

#[no_mangle]
pub extern "C" fn ANativeActivity_onCreate(
    activity: *mut std::ffi::c_void,
    _saved_state: *mut std::ffi::c_void,
    _saved_state_size: usize,
) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SysInfo"),
    );

    info!("SysInfo Dashboard Rust v0.2.0 starting");

    let lines = sysinfo::collect_system_info();
    for l in &lines {
        info!("{}", l);
    }

    // Create NativeActivity wrapper
    let na = unsafe { NativeActivity::from_raw(activity as *mut ndk_sys::ANativeActivity) };

    // Poll for window (up to 5 seconds)
    for attempt in 0..50 {
        if let Some(window) = na.window() {
            info!("Window available after {}ms", attempt * 100);
            render_to_window(&window, &lines);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Keep alive so the activity doesn't exit
    info!("Render complete, staying alive");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

fn render_to_window(window: &NativeWindow, lines: &[String]) {
    unsafe {
        let raw = window as *const _ as *mut ndk_sys::ANativeWindow;
        let mut buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
        let ret = ndk_sys::ANativeWindow_lock(raw, buf.as_mut_ptr(), std::ptr::null_mut());
        if ret != 0 {
            info!("ANativeWindow_lock failed: {}", ret);
            return;
        }

        let b = buf.assume_init();
        let w = b.width as usize;
        let h = b.height as usize;
        let stride = b.stride as usize;
        let pixels = b.bits as *mut u32;

        // Dark background
        for y in 0..h {
            for x in 0..w {
                *pixels.add(y * stride + x) = 0xFF121212u32;
            }
        }

        // Title
        draw_block_text(pixels, w, h, stride, "SysInfo Dashboard v0.2.0",
            (w / 2).saturating_sub(13 * 10), 16, 0xFF90CAF9u32);

        // Info lines
        let mut row = 50;
        let mut i = 0;
        while row + 16 < h && i < lines.len() {
            let line = &lines[i];
            if !line.is_empty() {
                let color = if line.starts_with("--") { 0xFF90CAF9u32 }
                    else if line.starts_with("  ") { 0xFFFFFFFFu32 }
                    else { 0xFF80FFFFFFu32 };
                draw_block_text(pixels, w, h, stride, line, 12, row, color);
                row += 18;
            } else {
                row += 10;
            }
            i += 1;
        }

        // Footer
        draw_block_text(pixels, w, h, stride, "Rust Native v0.2.0",
            w.saturating_sub(18 * 10), h.saturating_sub(24), 0xFF40FFFFFFu32);

        ndk_sys::ANativeWindow_unlockAndPost(raw);
        info!("Rendered OK ({}x{}, {} lines)", w, h, lines.len());
    }
}

fn draw_block_text(pixels: *mut u32, w: usize, h: usize, stride: usize,
                   text: &str, mut cx: usize, y: usize, color: u32) {
    for ch in text.chars() {
        if ch < ' ' { continue; }
        for dy in 0..14 {
            for dx in 0..8 {
                let px = cx + dx;
                let py = y + dy;
                if px < w && py < h {
                    unsafe { *pixels.add(py * stride + px) = color; }
                }
            }
        }
        cx += 10;
        if cx + 8 > w { break; }
    }
}
