use android_activity::*;
use log::{info, error};

mod android_api;
mod render;

#[derive(Default)]
struct AppState {
    info_lines: Vec<String>,
    renderer: render::TextRenderer,
}

fn main() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("SysInfo"),
    );

    let app = NativeApp::new();
    let mut state = AppState::default();

    app.run(|app, event| {
        match event {
            Event::Window(WindowEvent::Resize { width, height }) => {
                info!("Window resized: {}x{}", width, height);
                state.renderer.resize(width, height);
            }
            Event::Window(WindowEvent::Redraw { native_window }) => {
                // Gather info if we haven't yet
                if state.info_lines.is_empty() {
                    state.info_lines = android_api::collect_system_info(&app);
                }
                // Render
                state.renderer.render(&native_window, &state.info_lines);
                native_window.queue_buffer().ok();
            }
            Event::Window(WindowEvent::Destroy) => {
                info!("Window destroyed");
            }
            _ => {}
        }
    });
}
