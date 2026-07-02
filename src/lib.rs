use log::info;
use std::sync::Mutex;

mod sysinfo;

// Raw pointer wrapper that implements Send/Sync
struct WindowPtr(*mut std::ffi::c_void);
unsafe impl Send for WindowPtr {}
unsafe impl Sync for WindowPtr {}

static WINDOW_PTR: Mutex<Option<WindowPtr>> = Mutex::new(None);

extern "C" {
    fn ANativeWindow_lock(window: *mut std::ffi::c_void,
        out_buffer: *mut ANativeWindowBuffer, inout_dirty: *const std::ffi::c_void) -> i32;
    fn ANativeWindow_unlockAndPost(window: *mut std::ffi::c_void) -> i32;
}

#[repr(C)]
struct ANativeWindowBuffer {

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

    info!("SysInfo Dashboard Rust v0.2.1 starting");

    let lines = sysinfo::collect_system_info();
    for l in &lines {
        info!("{}", l);
    }

    // Store the activity and set up callbacks so we can get the window
    unsafe {
        let act = activity as *mut ANativeActivity;
        // Set callbacks to capture the window pointer
        let callbacks = Box::into_raw(Box::new(ANativeActivityCallbacks {
            on_start: None,
            on_resume: None,
            on_pause: None,
            on_stop: None,
            on_destroy: Some(on_destroy),
            on_native_window_created: Some(on_window_created),
            on_native_window_resized: Some(on_window_resized),
            on_native_window_redraw_needed: Some(on_window_redraw),
            on_native_window_destroyed: Some(on_window_destroyed),
        }));
        (*act).callbacks = callbacks as *mut _;
    }

    // Wait for window and render
    for _ in 0..100 {
        if let Some(win_ptr) = *WINDOW_PTR.lock().unwrap() {
            render_all(win_ptr.0, &lines);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Keep alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[repr(C)]
struct ANativeActivity {
    callbacks: *mut ANativeActivityCallbacks,
    _instance: *mut std::ffi::c_void,
    _cookie: *mut std::ffi::c_void,
    window: *mut std::ffi::c_void,
    _input_queue: *mut std::ffi::c_void,
    _clazz: *mut std::ffi::c_void,
    _destroy_requested: i32,
}

#[repr(C)]
struct ANativeActivityCallbacks {
    on_start: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_resume: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_pause: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_stop: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_destroy: Option<unsafe extern "C" fn(*mut ANativeActivity)>,
    on_native_window_created: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut std::ffi::c_void)>,
    on_native_window_resized: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut std::ffi::c_void)>,
    on_native_window_redraw_needed: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut std::ffi::c_void)>,
    on_native_window_destroyed: Option<unsafe extern "C" fn(*mut ANativeActivity, *mut std::ffi::c_void)>,
}

unsafe extern "C" fn on_window_created(_act: *mut ANativeActivity, window: *mut std::ffi::c_void) {
    info!("Window created");
    *WINDOW_PTR.lock().unwrap() = Some(WindowPtr(window));
}

unsafe extern "C" fn on_window_resized(_act: *mut ANativeActivity, window: *mut std::ffi::c_void) {
    info!("Window resized");
    *WINDOW_PTR.lock().unwrap() = Some(WindowPtr(window));
}

unsafe extern "C" fn on_window_redraw(_act: *mut ANativeActivity, window: *mut std::ffi::c_void) {
    info!("Window redraw needed");
}

unsafe extern "C" fn on_window_destroyed(_act: *mut ANativeActivity, _window: *mut std::ffi::c_void) {
    info!("Window destroyed");
    *WINDOW_PTR.lock().unwrap() = None;
}

unsafe extern "C" fn on_destroy(activity: *mut ANativeActivity) {
    info!("Destroy");
    if !(*activity).callbacks.is_null() {
        drop(Box::from_raw((*activity).callbacks));
        (*activity).callbacks = std::ptr::null_mut();
    }
}

fn render_all(window: *mut std::ffi::c_void, lines: &[String]) {
    info!("Rendering...");
    unsafe {
        let mut buf = std::mem::zeroed::<ANativeWindowBuffer>();
        let ret = ANativeWindow_lock(window, &mut buf as *mut _, std::ptr::null());
        if ret != 0 {
            info!("lock failed: {}", ret);
            return;
        }
        let w = buf.width as usize;
        let h = buf.height as usize;
        let stride = buf.stride as usize;
        let pixels = buf.bits as *mut u32;

        // Dark background
        for y in 0..h {
            for x in 0..w {
                *pixels.add(y * stride + x) = 0xFF1A1A1Au32;
            }
        }

        // Draw text as blocks
        let mut row = 20i32;
        draw_block(pixels, w, h, stride, "SysInfo Dashboard v0.2.1", &mut row, 0xFF90CAF9);
        for l in lines {
            if row as usize + 18 > h { break; }
            if l.is_empty() {
                row += 12;
                continue;
            }
            let color = if l.starts_with("--") { 0xFF90CAF9 }
                        else if l.starts_with("  ") { 0xFFFFFFFF }
                        else { 0xFF80FFFFFF };
            draw_block(pixels, w, h, stride, l, &mut row, color);
        }
        ANativeWindow_unlockAndPost(window);
        info!("Render done ({}x{})", w, h);
    }
}

fn draw_block(pixels: *mut u32, w: usize, h: usize, stride: usize,
              text: &str, row: &mut i32, color: u32) {
    let mut col = 12;
    for ch in text.chars() {
        if ch < ' ' { col += 8; continue; }
        let bw = 7usize;
        let bh = 11usize;
        unsafe {
            for dy in 0..bh {
                for dx in 0..bw {
                    let px = col + dx;
                    let py = *row as usize + dy;
                    if px < w && py < h {
                        *pixels.add(py * stride + px) = color;
                    }
                }
            }
        }
        col += 9;
        if col + 9 > w { break; }
    }
    *row += 16;
}
