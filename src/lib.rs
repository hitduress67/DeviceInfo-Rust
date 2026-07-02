use log::info;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod sysinfo;

static WINDOW: std::sync::Mutex<*mut std::ffi::c_void> = std::sync::Mutex::new(std::ptr::null_mut());
static RUNNING: AtomicBool = AtomicBool::new(true);

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

    // Collect system info
    let lines = sysinfo::collect_system_info();
    for l in &lines {
        info!("{}", l);
    }

    // Store the activity pointer and set up a simple callback
    // We use the native_app_glue style by storing the ANativeActivity pointer
    // and reading its `window` field to know when a window is available

    // Spawn a render thread
    let activity_ptr = activity as usize;
    std::thread::spawn(move || {
        info!("Render thread started");
        let mut rendered = false;

        while RUNNING.load(Ordering::Relaxed) {
            unsafe {
                let activity = activity_ptr as *mut ndk_sys::ANativeActivity;
                if activity.is_null() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                let window = (*activity).window;
                if window.is_null() {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                if !rendered {
                    info!("Window available, rendering...");
                    render_text(window, &lines);
                    rendered = true;
                }

                // Check if window was destroyed
                if (*activity).destroyRequested != 0 {
                    info!("Destroy requested");
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        info!("Render thread exiting");
    });
}

unsafe fn render_text(window: *mut ndk_sys::ANativeWindow, lines: &[String]) {
    let mut buf = std::mem::MaybeUninit::<ndk_sys::ANativeWindow_Buffer>::uninit();
    let ret = ndk_sys::ANativeWindow_lock(window, buf.as_mut_ptr(), std::ptr::null_mut());
    if ret != 0 {
        info!("lock failed: {}", ret);
        return;
    }

    let b = buf.assume_init();
    let w = b.width as usize;
    let h = b.height as usize;
    let stride = b.stride as usize;
    let pixels = b.bits as *mut u32;

    // Fill dark background
    for y in 0..h {
        for x in 0..w {
            *pixels.add(y * stride + x) = 0xFF121212u32;
        }
    }

    // Draw title
    let title = "SysInfo Dashboard v0.2.0";
    let tw = title.len() * 12;
    let tx = (w.saturating_sub(tw)) / 2;
    draw_text_simple(pixels, w, h, stride, title, tx, 16, 0xFF90CAF9u32);

    // Draw info lines
    let mut row = 50;
    for line in lines {
        if row + 18 > h { break; }
        let color = if line.starts_with("--") { 0xFF90CAF9u32 }
                    else if line.starts_with("  ") { 0xFFFFFFFFu32 }
                    else { 0xFF80FFFFFFu32 };
        draw_text_simple(pixels, w, h, stride, line, 12, row, color);
        row += if line.is_empty() { 10 } else { 18 };
    }

    // Footer
    let footer = "Rust Native";
    let fw = footer.len() * 12;
    draw_text_simple(pixels, w, h, stride, footer, w.saturating_sub(fw) - 12, h - 24, 0xFF40FFFFFFu32);

    ndk_sys::ANativeWindow_unlockAndPost(window);
    info!("Rendered {} lines", lines.len());
}

fn draw_text_simple(pixels: *mut u32, w: usize, h: usize, stride: usize,
                    text: &str, x: usize, y: usize, color: u32) {
    unsafe {
        let mut cx = x;
        for ch in text.chars() {
            if ch < ' ' { continue; }
            let bw = 8usize;
            let bh = 14usize;
            // Draw a colored block for each character (placeholder rendering)
            for dy in 0..bh {
                for dx in 0..bw {
                    let px = cx + dx;
                    let py = y + dy;
                    if px < w && py < h {
                        *pixels.add(py * stride + px) = color;
                    }
                }
            }
            cx += bw + 2;
            if cx + bw > w { break; }
        }
    }
}
