use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use std::f32::consts::PI;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vowel {
    A,
    E,
    I,
    O,
    U,
}

const TABLE: [(Vowel, &str, &str, f32, f32); 5] = [
    (Vowel::A, "vowel_a_image", "A", 750.0, 1300.0),
    (Vowel::E, "vowel_e_image", "E", 450.0, 2100.0),
    (Vowel::I, "vowel_i_image", "I", 270.0, 2500.0),
    (Vowel::O, "vowel_o_image", "O", 520.0, 950.0),
    (Vowel::U, "vowel_u_image", "U", 280.0, 650.0),
];

impl Vowel {
    pub const ALL: [Vowel; 5] = [Vowel::A, Vowel::E, Vowel::I, Vowel::O, Vowel::U];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn settings_key(self) -> &'static str {
        TABLE[self.index()].1
    }

    pub fn label(self) -> &'static str {
        TABLE[self.index()].2
    }
}

pub fn nearest_vowel(f1: f32, f2: f32) -> Vowel {
    TABLE
        .iter()
        .min_by(|(_, _, _, a1, a2), (_, _, _, b1, b2)| {
            let da = (f1 - a1).powi(2) + 0.3 * (f2 - a2).powi(2);
            let db = (f1 - b1).powi(2) + 0.3 * (f2 - b2).powi(2);
            da.total_cmp(&db)
        })
        .map(|(v, ..)| *v)
        .unwrap()
}

fn peak_frequency(magnitudes: &[f32], bin_hz: f32, min_hz: f32, max_hz: f32) -> Option<f32> {
    let start = (min_hz / bin_hz).floor().max(1.0) as usize;
    let end = ((max_hz / bin_hz).ceil() as usize).min(magnitudes.len().saturating_sub(1));
    if start >= end {
        return None;
    }

    let mut best_bin = start;
    let mut best_mag = magnitudes[start];
    for (i, &mag) in magnitudes.iter().enumerate().take(end + 1).skip(start + 1) {
        if mag > best_mag {
            best_mag = mag;
            best_bin = i;
        }
    }

    Some(best_bin as f32 * bin_hz)
}

pub struct VowelAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    size: usize,
    window: Vec<f32>,
}

impl VowelAnalyzer {
    pub fn new(size: usize) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(size);
        let window = (0..size)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (size as f32 - 1.0)).cos())
            .collect();
        Self { fft, size, window }
    }

    pub fn size(&self) -> usize {
        self.size
    }
    pub fn formants(&self, samples: &[f32], sample_rate: u32) -> Option<(f32, f32)> {
        if samples.len() != self.size || sample_rate == 0 {
            return None;
        }

        let mut buffer: Vec<Complex32> = samples
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| Complex32::new(s * w, 0.0))
            .collect();
        self.fft.process(&mut buffer);

        let magnitudes: Vec<f32> = buffer
            .iter()
            .take(self.size / 2)
            .map(|c| c.norm())
            .collect();
        let bin_hz = sample_rate as f32 / self.size as f32;

        let f1 = peak_frequency(&magnitudes, bin_hz, 150.0, 1000.0)?;
        let f2 = peak_frequency(&magnitudes, bin_hz, f1 + 150.0, 3200.0)?;

        Some((f1, f2))
    }
}
