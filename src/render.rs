use ndk_sys::ANativeWindow;
use std::ffi::c_void;

const FONT_W: usize = 5;
const FONT_H: usize = 8;
static FONT: &[u8] = include_bytes!("font_5x8.bin");

const BG: u32 = 0xFF121212;
const FG: u32 = 0xFFFFFFFF;
const ACCENT: u32 = 0xFF90CAF9;

pub struct TextRenderer {
    buf: Vec<u32>,
    w: usize,
    h: usize,
    scale: usize,
    col: usize,
    row: usize,
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            w: 0, h: 0, scale: 2,
            col: 0, row: 0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.w = width;
        self.h = height;
        self.buf = vec![BG; width * height];
        self.scale = (width / (48 * FONT_W)).max(1);
        self.col = 2 * self.scale;
        self.row = 2 * self.scale;
    }

    /// Render using NativeWindow from android-activity
    pub fn render_raw(&mut self, window: &ndk::native_window::NativeWindow, lines: &[String]) {
        // Try to get the raw ANativeWindow pointer
        let raw_ptr = window as *const _ as *const ANativeWindow;
        self.render(raw_ptr, lines);
    }

    pub fn render(&mut self, window: &ANativeWindow, lines: &[String]) {
        self.buf.fill(BG);
        self.col = 2 * self.scale;
        self.row = 2 * self.scale;

        // Lock window buffer
        let mut out_buffer = std::ptr::null_mut();
        let mut out_format = 0;
        let mut out_width = 0i32;
        let mut out_height = 0i32;
        let mut out_stride = 0i32;
        let mut out_pixels: *mut c_void = std::ptr::null_mut();

        unsafe {
            ANativeWindow_lock(
                window,
                &mut out_buffer as *mut *mut _,
                std::ptr::null_mut(),
            );
            if !out_buffer.is_null() {
                let buf = &*(out_buffer as *const ndk_sys::ARect);
                out_width = buf.right - buf.left;
                out_height = buf.bottom - buf.top;
                out_stride = out_width; // simplified
                // Get pixel pointer (ANativeWindow_Buffer.bits)
                let bits_offset = 0; // offset of bits field in ANativeWindow_Buffer
                out_pixels = *(out_buffer as *const *mut c_void).add(1); // hack
            }
        }

        // Fallback: use last known dimensions if lock failed
        if out_pixels.is_null() {
            // Draw to internal buffer and skip window blit
            self.render_to_buffer(lines);
            return;
        }

        self.render_to_buffer(lines);

        // Copy buffer to window
        let width = out_width as usize;
        let height = out_height as usize;
        let stride = out_stride as usize;
        unsafe {
            let bytes = std::slice::from_raw_parts_mut(
                out_pixels as *mut u8,
                height * stride * 4,
            );
            for y in 0..height.min(self.h) {
                let src_off = y * self.w;
                let dst_off = y * stride;
                for x in 0..width.min(self.w) {
                    let px = self.buf[src_off + x];
                    let off = dst_off + x * 4;
                    bytes[off] = ((px >> 16) & 0xFF) as u8;     // R
                    bytes[off + 1] = ((px >> 8) & 0xFF) as u8;  // G
                    bytes[off + 2] = ((px >> 0) & 0xFF) as u8;  // B
                    bytes[off + 3] = ((px >> 24) & 0xFF) as u8; // A
                }
            }
        }

        unsafe {
            ANativeWindow_unlockAndPost(window);
        }
    }

    fn render_to_buffer(&mut self, lines: &[String]) {
        self.col = 2 * self.scale;
        self.row = 2 * self.scale;

        let status = "SysInfo Dashboard  v0.1.0";
        self.draw_str(status, ACCENT);

        self.row = FONT_H * self.scale + 4;

        for line in lines {
            if line.is_empty() {
                self.row += FONT_H * self.scale / 2;
                continue;
            }
            let color = if line.starts_with("──") { ACCENT } else { FG };
            let clean = if line.starts_with("──") {
                line.trim_matches('─').trim()
            } else {
                line
            };
            self.draw_str(clean, color);
            self.row += FONT_H * self.scale + 2;
            if self.row + FONT_H * self.scale >= self.h {
                break;
            }
        }

        self.row = self.h - FONT_H * self.scale - 4;
        self.draw_str("  Rust Native  |  Swipe to refresh", 0x40FFFFFF);
    }

    fn draw_str(&mut self, text: &str, color: u32) {
        let mut cx = self.col;
        let cy = self.row;
        for ch in text.chars() {
            let idx = (ch as usize).saturating_sub(32);
            if idx >= 95 {
                cx += FONT_W * self.scale;
                continue;
            }
            let glyph_off = idx * FONT_H;
            for gy in 0..FONT_H {
                if glyph_off + gy >= FONT.len() { break; }
                let row_bits = FONT[glyph_off + gy];
                for gx in 0..FONT_W {
                    if row_bits & (1 << (4 - gx)) != 0 {
                        for sy in 0..self.scale {
                            for sx in 0..self.scale {
                                let px = cx + gx * self.scale + sx;
                                let py = cy + gy * self.scale + sy;
                                if px < self.w && py < self.h {
                                    self.buf[py * self.w + px] = color;
                                }
                            }
                        }
                    }
                }
            }
            cx += (FONT_W + 1) * self.scale;
        }
    }
}

extern "C" {
    fn ANativeWindow_lock(
        window: *const ANativeWindow,
        outBuffer: *mut *mut std::ffi::c_void,
        inOutDirtyBounds: *const std::ffi::c_void,
    ) -> i32;
    fn ANativeWindow_unlockAndPost(window: *const ANativeWindow) -> i32;
}
