use log::info;

mod android_api;

/// Entry point called by android_native_app_glue or cargo-apk
#[no_mangle]
pub extern "C" fn ANativeActivity_onCreate(
    activity: *mut ndk_sys::ANativeActivity,
    saved_state: *mut std::ffi::c_void,
    saved_state_size: usize,
) {
    use ndk::native_activity::NativeActivity;
    
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SysInfo"),
    );

    info!("SysInfo Dashboard Rust v0.1.0 starting");

    // Store NativeActivity reference globally for JNI
    let na = NativeActivity::new(activity);
    let vm = na.vm();
    let jvm = vm.attach_current_thread();

    let info_lines = android_api::collect_system_info(&na);

    // Log the collected info (visible in logcat)
    for line in &info_lines {
        info!("{}", line);
    }
    
    info!("System info collection complete ({} lines)", info_lines.len());
    
    // In this simplified version, the output goes to logcat
    // Full window rendering requires the app glue event loop
    // which is too complex for this initial release
}
