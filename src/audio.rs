use crate::vowel::{nearest_vowel, Vowel, VowelAnalyzer};
use obs_wrapper::obs_sys::{
    audio_data, obs_get_source_by_name, obs_source_add_audio_capture_callback, obs_source_release,
    obs_source_remove_audio_capture_callback, obs_source_t,
};
use std::ffi::{c_void, CString};
use std::sync::{Arc, Mutex};

const FFT_SIZE: usize = 2048;

pub struct AudioState {
    pub level: f32,
    pub threshold: f32,
    pub vowel_enabled: bool,
    pub vowel_smoothing: f32,
    pub sample_rate: u32,
    pub vowel: Option<Vowel>,
    buffer: Vec<f32>,
    analyzer: VowelAnalyzer,
    smoothed_formants: Option<(f32, f32)>,
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            level: 0.0,
            threshold: 0.02,
            vowel_enabled: false,
            vowel_smoothing: 0.5,
            sample_rate: 48000,
            vowel: None,
            buffer: Vec::with_capacity(FFT_SIZE),
            analyzer: VowelAnalyzer::new(FFT_SIZE),
            smoothed_formants: None,
        }
    }

    fn process(&mut self, samples: &[f32]) {
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_sq / samples.len().max(1) as f32).sqrt();
        self.level = self.level * 0.6 + rms * 0.4;

        if self.level < self.threshold || !self.vowel_enabled {
            self.vowel = None;
            self.buffer.clear();
            self.smoothed_formants = None;
            return;
        }

        self.buffer.extend_from_slice(samples);
        let size = self.analyzer.size();
        if self.buffer.len() >= size {
            let excess = self.buffer.len() - size;
            if let Some((f1, f2)) = self
                .analyzer
                .formants(&self.buffer[excess..], self.sample_rate)
            {
                let (f1, f2) = match self.smoothed_formants {
                    Some((pf1, pf2)) => {
                        let a = self.vowel_smoothing;
                        (pf1 * a + f1 * (1.0 - a), pf2 * a + f2 * (1.0 - a))
                    }
                    None => (f1, f2),
                };
                self.smoothed_formants = Some((f1, f2));
                self.vowel = Some(nearest_vowel(f1, f2));
            }
            self.buffer.clear();
        }
    }
}

unsafe extern "C" fn audio_capture_callback(
    param: *mut c_void,
    _source: *mut obs_source_t,
    audio: *const audio_data,
    _muted: bool,
) {
    if param.is_null() || audio.is_null() {
        return;
    }
    let audio = &*audio;
    if audio.frames == 0 || audio.data[0].is_null() {
        return;
    }
    let samples = std::slice::from_raw_parts(audio.data[0] as *const f32, audio.frames as usize);
    let state = &*(param as *const Mutex<AudioState>);
    if let Ok(mut state) = state.lock() {
        state.process(samples);
    }
}

pub struct MicSubscription {
    source: *mut obs_source_t,
    state: *const Mutex<AudioState>,
    attached: bool,
}

impl MicSubscription {
    pub fn new(name: &str, state: &Arc<Mutex<AudioState>>) -> Option<Self> {
        if name.is_empty() {
            return None;
        }
        let cname = CString::new(name).ok()?;
        let source = unsafe { obs_get_source_by_name(cname.as_ptr()) };
        if source.is_null() {
            return None;
        }
        let state_ptr = Arc::into_raw(state.clone());
        unsafe {
            obs_source_add_audio_capture_callback(
                source,
                Some(audio_capture_callback),
                state_ptr as *mut c_void,
            );
        }
        Some(Self {
            source,
            state: state_ptr,
            attached: true,
        })
    }

    pub fn pause(&mut self) {
        if !self.attached {
            return;
        }
        unsafe {
            obs_source_remove_audio_capture_callback(
                self.source,
                Some(audio_capture_callback),
                self.state as *mut c_void,
            );
        }
        self.attached = false;
    }

    pub fn resume(&mut self) {
        if self.attached {
            return;
        }
        unsafe {
            obs_source_add_audio_capture_callback(
                self.source,
                Some(audio_capture_callback),
                self.state as *mut c_void,
            );
        }
        self.attached = true;
    }
}

impl Drop for MicSubscription {
    fn drop(&mut self) {
        unsafe {
            if self.attached {
                obs_source_remove_audio_capture_callback(
                    self.source,
                    Some(audio_capture_callback),
                    self.state as *mut c_void,
                );
            }
            obs_source_release(self.source);
            Arc::from_raw(self.state);
        }
    }
}
