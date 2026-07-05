use obs_wrapper::graphics::{GraphicsColorFormat, GraphicsTexture};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

pub struct ImageAsset {
    frames: Vec<GraphicsTexture>,
    cumulative_ms: Vec<u64>,
    total_ms: u64,
}

impl ImageAsset {
    pub fn current_texture(&self, elapsed: Duration) -> &GraphicsTexture {
        if self.frames.len() == 1 || self.total_ms == 0 {
            return &self.frames[0];
        }
        let t = elapsed.as_millis() as u64 % self.total_ms;
        let index = self
            .cumulative_ms
            .iter()
            .position(|&end| t < end)
            .unwrap_or(0);
        &self.frames[index]
    }
}

fn make_texture(image: &image::RgbaImage) -> GraphicsTexture {
    let (width, height) = image.dimensions();
    let mut texture = GraphicsTexture::new(width, height, GraphicsColorFormat::RGBA);
    texture.set_image(image.as_raw(), width * 4, false);
    texture
}

fn load_gif(path: &str) -> Option<ImageAsset> {
    use image::{codecs::gif::GifDecoder, AnimationDecoder};

    let file = File::open(path).ok()?;
    let decoder = GifDecoder::new(BufReader::new(file)).ok()?;

    let mut frames = Vec::new();
    let mut cumulative_ms = Vec::new();
    let mut total_ms: u64 = 0;

    for frame in decoder.into_frames() {
        let frame = frame.ok()?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let ms = if denom == 0 {
            100
        } else {
            (numer / denom).max(20) as u64
        };

        frames.push(make_texture(frame.buffer()));
        total_ms += ms;
        cumulative_ms.push(total_ms);
    }

    if frames.is_empty() {
        return None;
    }

    Some(ImageAsset {
        frames,
        cumulative_ms,
        total_ms,
    })
}

fn load_static(path: &str) -> Option<ImageAsset> {
    let image = image::open(path).ok()?.into_rgba8();
    if image.width() == 0 || image.height() == 0 {
        return None;
    }

    Some(ImageAsset {
        frames: vec![make_texture(&image)],
        cumulative_ms: vec![0],
        total_ms: 0,
    })
}

pub fn load_image_asset(path: &str) -> Option<ImageAsset> {
    if path.is_empty() {
        return None;
    }

    if path.to_ascii_lowercase().ends_with(".gif") {
        load_gif(path)
    } else {
        load_static(path)
    }
}
