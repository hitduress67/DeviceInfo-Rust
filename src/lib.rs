use log::info;

mod sysinfo;

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

    let lines = sysinfo::collect_system_info();
    for l in &lines {
        info!("{}", l);
    }

    info!("Info collection complete - {} lines. Enable 'Show logcat' to view.", lines.len());
    
    // Keep alive to prevent the process from exiting immediately
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
