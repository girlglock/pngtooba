use obs_wrapper::graphics::{GraphicsColorFormat, GraphicsTexture};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

pub struct ImageAsset {
    frames: Vec<GraphicsTexture>,
    cumulative_ms: Vec<u64>,
    total_ms: u64,
    total_plays: Option<u32>,
}

impl ImageAsset {
    pub fn current_texture(&self, elapsed: Duration) -> &GraphicsTexture {
        if self.frames.len() == 1 || self.total_ms == 0 {
            return &self.frames[0];
        }
        let elapsed_ms = elapsed.as_millis() as u64;
        let t = match self.total_plays {
            Some(plays) if elapsed_ms >= self.total_ms * plays as u64 => self.total_ms - 1,
            _ => elapsed_ms % self.total_ms,
        };
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

fn read_total_plays(path: &str) -> Option<u32> {
    let file = File::open(path).ok()?;
    let decoder = gif::Decoder::new(BufReader::new(file)).ok()?;
    match decoder.repeat() {
        gif::Repeat::Infinite => None,
        gif::Repeat::Finite(0) => Some(1),
        gif::Repeat::Finite(n) => Some(n as u32 + 1),
    }
}

fn load_gif(path: &str) -> Option<ImageAsset> {
    use image::{codecs::gif::GifDecoder, AnimationDecoder};

    let total_plays = read_total_plays(path);

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
        total_plays,
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
        total_plays: None,
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
