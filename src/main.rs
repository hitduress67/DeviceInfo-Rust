use android_activity::*;
use log::info;

mod android_api;
mod render;

fn main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SysInfo"),
    );

    info!("SysInfo Dashboard Rust v0.1.0 starting");
    
    let app = NativeApp::new();
    let mut renderer = render::TextRenderer::new();
    let mut info_lines: Vec<String> = Vec::new();

    // In pure Rust without Java Activity, ANativeWindow_lock might not work
    // as expected. We render to the internal buffer and rely on NativeActivity's
    // default rendering. Fallback: collect info and render to buffer.
    info_lines = android_api::collect_system_info(&app);

    app.run(|app, event| {
        match event {
            Event::Window(WindowEvent::Resize { width, height }) => {
                info!("Window resized: {}x{}", width, height);
                renderer.resize(width, height);
            }
            Event::Window(WindowEvent::Redraw { native_window }) => {
                renderer.render_raw(&native_window, &info_lines);
                native_window.queue_buffer().ok();
            }
            Event::Window(WindowEvent::Destroy) => {
                info!("Window destroyed");
            }
            _ => {}
        }
    });
}
