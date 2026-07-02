use log::info;
use std::ffi::c_void;

mod sysinfo;

#[no_mangle]
pub extern "C" fn ANativeActivity_onCreate(
    activity: *mut c_void,
    _saved_state: *mut c_void,
    _saved_state_size: usize,
) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SysInfo"),
    );

    info!("SysInfo Dashboard Rust v0.1.0 starting");

    let info_lines = sysinfo::collect_system_info();
    for line in &info_lines {
        info!("{}", line);
    }

    info!("Done - {} lines collected", info_lines.len());
}
