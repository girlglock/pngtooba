use obs_wrapper::graphics::{GraphicsColorFormat, GraphicsTexture};
use std::time::{Duration, Instant};

const OVERLAY_WIDTH: u32 = 132;
const OVERLAY_HEIGHT: u32 = 84;
const REBUILD_INTERVAL: Duration = Duration::from_millis(150);
const SMOOTHING: f64 = 0.15;

const FONT_W: usize = 3;
const FONT_H: usize = 5;

fn glyph(c: char) -> [&'static str; FONT_H] {
    match c.to_ascii_uppercase() {
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["###", "..#", "###", "#..", "###"],
        '3' => ["###", "..#", "###", "..#", "###"],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "###", "..#", "###"],
        '6' => ["###", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", "..#", "..#", "..#"],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "###"],
        'A' => [".#.", "#.#", "###", "#.#", "#.#"],
        'D' => ["##.", "#.#", "#.#", "#.#", "##."],
        'F' => ["###", "#..", "##.", "#..", "#.."],
        'M' => ["#.#", "###", "#.#", "#.#", "#.#"],
        'N' => ["#.#", "##.", "###", ".##", "#.#"],
        'O' => ["###", "#.#", "#.#", "#.#", "###"],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'S' => ["###", "#..", "###", "..#", "###"],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'W' => ["#.#", "#.#", "#.#", "###", "#.#"],
        'X' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
        ':' => ["...", ".#.", "...", ".#.", "..."],
        '.' => ["...", "...", "...", "...", ".#."],
        _ => ["...", "...", "...", "...", "..."],
    }
}

fn draw_text(buf: &mut image::RgbaImage, text: &str, x0: i32, y0: i32, scale: i32, color: [u8; 4]) {
    for (i, ch) in text.chars().enumerate() {
        let gx = x0 + i as i32 * (FONT_W as i32 + 1) * scale;
        for (row, line) in glyph(ch).iter().enumerate() {
            for (col, cell) in line.chars().enumerate() {
                if cell != '#' {
                    continue;
                }
                let px = gx + col as i32 * scale;
                let py = y0 + row as i32 * scale;
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = px + dx;
                        let y = py + dy;
                        if x >= 0 && y >= 0 && (x as u32) < buf.width() && (y as u32) < buf.height()
                        {
                            buf.put_pixel(x as u32, y as u32, image::Rgba(color));
                        }
                    }
                }
            }
        }
    }
}

fn ema(prev: f64, sample_ns: f64) -> f64 {
    if prev <= 0.0 {
        sample_ns
    } else {
        prev + (sample_ns - prev) * SMOOTHING
    }
}

fn fmt_ns(ns: f64) -> String {
    if ns >= 1_000_000.0 {
        format!("{:.1}MS", ns / 1_000_000.0)
    } else if ns >= 1_000.0 {
        format!("{:.0}US", ns / 1_000.0)
    } else {
        format!("{:.0}NS", ns)
    }
}

pub struct FrameProfiler {
    audio_ns: f64,
    effect_ns: f64,
    draw_ns: f64,
    total_ns: f64,
    texture: Option<GraphicsTexture>,
    last_rebuild: Instant,
}

impl FrameProfiler {
    pub fn new() -> Self {
        Self {
            audio_ns: 0.0,
            effect_ns: 0.0,
            draw_ns: 0.0,
            total_ns: 0.0,
            texture: None,
            last_rebuild: Instant::now() - REBUILD_INTERVAL,
        }
    }

    pub fn record(&mut self, audio: Duration, effect: Duration, draw: Duration, total: Duration) {
        self.audio_ns = ema(self.audio_ns, audio.as_nanos() as f64);
        self.effect_ns = ema(self.effect_ns, effect.as_nanos() as f64);
        self.draw_ns = ema(self.draw_ns, draw.as_nanos() as f64);
        self.total_ns = ema(self.total_ns, total.as_nanos() as f64);
    }

    pub fn overlay(&mut self) -> &GraphicsTexture {
        if self.texture.is_none() || self.last_rebuild.elapsed() >= REBUILD_INTERVAL {
            self.rebuild_texture();
            self.last_rebuild = Instant::now();
        }
        self.texture.as_ref().unwrap()
    }

    fn rebuild_texture(&mut self) {
        let mut img = image::RgbaImage::from_pixel(
            OVERLAY_WIDTH,
            OVERLAY_HEIGHT,
            image::Rgba([0, 0, 0, 170]),
        );
        let color = [80, 255, 120, 255];
        draw_text(
            &mut img,
            &format!("AUD {}", fmt_ns(self.audio_ns)),
            4,
            4,
            2,
            color,
        );
        draw_text(
            &mut img,
            &format!("FX  {}", fmt_ns(self.effect_ns)),
            4,
            22,
            2,
            color,
        );
        draw_text(
            &mut img,
            &format!("DRW {}", fmt_ns(self.draw_ns)),
            4,
            40,
            2,
            color,
        );
        draw_text(
            &mut img,
            &format!("TOT {}", fmt_ns(self.total_ns)),
            4,
            58,
            2,
            color,
        );

        match self.texture.as_mut() {
            Some(tex) => tex.set_image(img.as_raw(), OVERLAY_WIDTH * 4, false),
            None => {
                let mut tex =
                    GraphicsTexture::new(OVERLAY_WIDTH, OVERLAY_HEIGHT, GraphicsColorFormat::RGBA);
                tex.set_image(img.as_raw(), OVERLAY_WIDTH * 4, false);
                self.texture = Some(tex);
            }
        }
    }
}
