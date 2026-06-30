use ndk::native_window::NativeWindow;

const FONT_W: usize = 5;
const FONT_H: usize = 8;
/// 5x8 bitmap font for ASCII 32-126
static FONT: &[u8] = include_bytes!("font_5x8.bin");

const BG: u32 = 0xFF121212;
const FG: u32 = 0xFFFFFFFF;
const LABEL: u32 = 0xFF80FFFFFF;
const ACCENT: u32 = 0xFF90CAF9;
const HEADER_BG: u32 = 0xFF1A1A2E;
const SECTION: u32 = 0xFF1565C0;
const GREEN: u32 = 0xFF4CAF50;

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
            w: 0,
            h: 0,
            scale: 2,
            col: 0,
            row: 0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.w = width;
        self.h = height;
        self.buf = vec![BG; width * height];
        // Auto-scale: target ~48 chars wide
        let target_cols = 48;
        self.scale = (width / (target_cols * FONT_W)).max(1);
        self.col = 2 * self.scale;
        self.row = 2 * self.scale;
    }

    pub fn render(&mut self, window: &NativeWindow, lines: &[String]) {
        self.buf.fill(BG);
        self.col = 2 * self.scale;
        self.row = 2 * self.scale;

        // Status bar
        let status = "SysInfo Dashboard  v1.0.0-rc1";
        self.draw_str(status, 0, 0, ACCENT, HEADER_BG);
        self.row = 14;

        for line in lines {
            if line.is_empty() {
                self.row += FONT_H * self.scale / 2;
                continue;
            }

            if line.starts_with("──") {
                // Section header
                let clean = line.trim_matches('─').trim();
                self.draw_str(clean, 0, 0, ACCENT, BG);
                self.row += 4;
            } else if line.starts_with("  ") {
                // Key: Value line
                self.draw_str(&line, 0, 0, FG, BG);
            } else {
                self.draw_str(&line, 0, 0, FG, BG);
            }

            self.row += FONT_H * self.scale + 2;
            if self.row + FONT_H * self.scale >= self.h {
                break;
            }
        }

        // Footer
        self.row = self.h - FONT_H * self.scale - 4;
        self.draw_str("  Rust Native  |  Swipe to refresh", 0, 0, 0x40FFFFFF, BG);

        // Write buffer to window
        let width = window.width() as usize;
        let height = window.height() as usize;
        let pitch = window.format_pitch() as usize;
        let bytes = unsafe { std::slice::from_raw_parts_mut(
            window.buffer_mut().unwrap_or(std::ptr::null_mut()) as *mut u8,
            height * pitch,
        )};

        for y in 0..height.min(self.h) {
            let src_row = y * self.w;
            let dst_off = y * pitch;
            for x in 0..width.min(self.w) {
                let px = self.buf[src_row + x];
                bytes[dst_off + x * 4 + 0] = (px >> 0) as u8;  // R
                bytes[dst_off + x * 4 + 1] = (px >> 8) as u8;  // G
                bytes[dst_off + x * 4 + 2] = (px >> 16) as u8; // B
                bytes[dst_off + x * 4 + 3] = (px >> 24) as u8; // A
            }
        }
    }

    fn draw_str(&mut self, text: &str, _x: usize, _y: usize, color: u32, _bg: u32) {
        let mut cx = self.col;
        let cy = self.row;
        for ch in text.chars() {
            let idx = (ch as usize).saturating_sub(32);
            if idx >= 95 {
                cx += FONT_W * self.scale;
                continue;
            }
            let glyph_offset = idx * FONT_H;
            for gy in 0..FONT_H {
                if glyph_offset + gy >= FONT.len() { break; }
                let row_bits = FONT[glyph_offset + gy];
                for gx in 0..FONT_W {
                    if row_bits & (1 << (4 - gx)) != 0 {
                        // Draw scaled pixel
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
            cx += (FONT_W as usize + 1) * self.scale;
        }
    }
}
