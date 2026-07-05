use crate::audio::{AudioState, MicSubscription};
use crate::enumerate::list_audio_sources;
use crate::image_asset::{load_image_asset, ImageAsset};
use crate::properties_ext::{add_group, set_visible, set_visible_raw};
use crate::vowel::Vowel;
use obs_wrapper::{
    data::DataObj,
    obs_string,
    obs_sys::{
        obs_data_release, obs_data_set_string, obs_data_t, obs_properties_add_button,
        obs_properties_t, obs_property_set_modified_callback, obs_property_t,
        obs_source_get_settings, obs_source_t, obs_source_update,
    },
    properties::*,
    source::*,
    string::ObsString,
    wrapper::PtrWrapper,
};
use std::borrow::Cow;
use std::ffi::{c_void, CString};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const EFFECT_NONE: &str = "none";
const EFFECT_SHAKE: &str = "shake";
const EFFECT_BOUNCE: &str = "bounce";

fn get_string(settings: &DataObj, key: &str) -> String {
    settings
        .get::<Cow<str>>(key)
        .map(|s| s.into_owned())
        .unwrap_or_default()
}

fn reload_if_changed(
    path: &mut String,
    asset: &mut Option<ImageAsset>,
    settings: &DataObj,
    key: &str,
) {
    let new_path = get_string(settings, key);
    if new_path != *path {
        *asset = load_image_asset(&new_path);
        *path = new_path;
    }
}

fn dimension_prop() -> NumberProp<i64> {
    NumberProp::<i64>::new_int().with_range(1..=8192)
}

fn image_path_prop() -> PathProp {
    PathProp::new(PathType::File).with_filter(obs_string!("Image Files (*.png *.gif *.webp)"))
}

unsafe fn clear_source_field(data: *mut c_void, key: &str) -> bool {
    let source = data as *mut obs_source_t;
    if source.is_null() {
        return false;
    }
    let settings = obs_source_get_settings(source);
    if settings.is_null() {
        return false;
    }
    let cname = CString::new(key).unwrap();
    let empty = CString::new("").unwrap();
    obs_data_set_string(settings, cname.as_ptr(), empty.as_ptr());
    obs_source_update(source, std::ptr::null_mut());
    obs_data_release(settings);
    true
}

macro_rules! clear_callback {
    ($fn_name:ident, $key:literal) => {
        unsafe extern "C" fn $fn_name(
            _props: *mut obs_properties_t,
            _property: *mut obs_property_t,
            data: *mut c_void,
        ) -> bool {
            clear_source_field(data, $key)
        }
    };
}

clear_callback!(clear_idle_image, "idle_image");
clear_callback!(clear_speaking_image, "speaking_image");
clear_callback!(clear_overlay_image, "overlay_image");
clear_callback!(clear_vowel_a_image, "vowel_a_image");
clear_callback!(clear_vowel_e_image, "vowel_e_image");
clear_callback!(clear_vowel_i_image, "vowel_i_image");
clear_callback!(clear_vowel_o_image, "vowel_o_image");
clear_callback!(clear_vowel_u_image, "vowel_u_image");

type ClearCallback =
    unsafe extern "C" fn(*mut obs_properties_t, *mut obs_property_t, *mut c_void) -> bool;

fn add_image_field(g: &mut Properties, key: &str, label: ObsString, clear_callback: ClearCallback) {
    g.add(ObsString::from(key), label, image_path_prop());

    let button_name = CString::new(format!("{key}_clear_btn")).unwrap();
    let button_text = CString::new("Clear").unwrap();
    unsafe {
        obs_properties_add_button(
            g.as_ptr_mut(),
            button_name.as_ptr(),
            button_text.as_ptr(),
            Some(clear_callback),
        );
    }
}

pub struct PngTuberSource {
    width: u32,
    height: u32,
    animation_start: Instant,
    keep_idle_visible: bool,

    idle_path: String,
    idle_asset: Option<ImageAsset>,

    speaking_path: String,
    speaking_asset: Option<ImageAsset>,

    vowel_paths: [String; 5],
    vowel_assets: [Option<ImageAsset>; 5],

    overlay_path: String,
    overlay_asset: Option<ImageAsset>,

    when_speaking_effect: String,
    shake_intensity: f32,
    shake_speed: f32,
    bounce_height: f32,
    bounce_speed: f32,

    mic_source_name: String,
    mic: Option<MicSubscription>,
    audio_state: Arc<Mutex<AudioState>>,
}

impl PngTuberSource {
    fn new() -> Self {
        Self {
            width: 512,
            height: 512,
            animation_start: Instant::now(),
            keep_idle_visible: false,
            idle_path: String::new(),
            idle_asset: None,
            speaking_path: String::new(),
            speaking_asset: None,
            vowel_paths: Default::default(),
            vowel_assets: Default::default(),
            overlay_path: String::new(),
            overlay_asset: None,
            when_speaking_effect: EFFECT_NONE.to_string(),
            shake_intensity: 8.0,
            shake_speed: 10.0,
            bounce_height: 25.0,
            bounce_speed: 2.5,
            mic_source_name: String::new(),
            mic: None,
            audio_state: Arc::new(Mutex::new(AudioState::new())),
        }
    }

    fn apply_settings(&mut self, settings: &mut DataObj, global: &GlobalContext) {
        self.width = settings.get::<i64>("width").unwrap_or(512).clamp(1, 8192) as u32;
        self.height = settings.get::<i64>("height").unwrap_or(512).clamp(1, 8192) as u32;
        self.keep_idle_visible = settings.get::<bool>("keep_idle_visible").unwrap_or(false);

        reload_if_changed(
            &mut self.idle_path,
            &mut self.idle_asset,
            settings,
            "idle_image",
        );
        reload_if_changed(
            &mut self.speaking_path,
            &mut self.speaking_asset,
            settings,
            "speaking_image",
        );

        for vowel in Vowel::ALL {
            let i = vowel.index();
            reload_if_changed(
                &mut self.vowel_paths[i],
                &mut self.vowel_assets[i],
                settings,
                vowel.settings_key(),
            );
        }

        reload_if_changed(
            &mut self.overlay_path,
            &mut self.overlay_asset,
            settings,
            "overlay_image",
        );

        self.when_speaking_effect = get_string(settings, "when_speaking_effect");
        if self.when_speaking_effect.is_empty() {
            self.when_speaking_effect = EFFECT_NONE.to_string();
        }
        self.shake_intensity = settings.get::<f64>("shake_intensity").unwrap_or(8.0) as f32;
        self.shake_speed = settings.get::<f64>("shake_speed").unwrap_or(10.0) as f32;
        self.bounce_height = settings.get::<f64>("bounce_height").unwrap_or(25.0) as f32;
        self.bounce_speed = settings.get::<f64>("bounce_speed").unwrap_or(2.5) as f32;

        let vowel_enabled = settings.get::<bool>("vowel_enabled").unwrap_or(false);
        let vowel_smoothing = settings.get::<f64>("vowel_smoothing").unwrap_or(0.5) as f32;
        let sensitivity = settings.get::<f64>("mic_sensitivity").unwrap_or(0.02) as f32;
        let sample_rate = global.with_audio(|audio| audio.output_sample_rate() as u32);

        {
            let mut state = self.audio_state.lock().unwrap();
            state.vowel_enabled = vowel_enabled;
            state.vowel_smoothing = vowel_smoothing;
            state.threshold = sensitivity;
            state.sample_rate = sample_rate;
        }

        let mic_source_name = get_string(settings, "mic_source");
        if mic_source_name != self.mic_source_name {
            self.mic = MicSubscription::new(&mic_source_name, &self.audio_state);
            self.mic_source_name = mic_source_name;
        }
    }
}

impl Sourceable for PngTuberSource {
    fn get_id() -> ObsString {
        obs_string!("pngtooba_source")
    }

    fn get_type() -> SourceType {
        SourceType::INPUT
    }

    fn create(create: &mut CreatableSourceContext<Self>, _source: SourceContext) -> Self {
        let mut source = Self::new();
        source.apply_settings(&mut create.settings, &GlobalContext);
        source
    }
}

impl GetNameSource for PngTuberSource {
    fn get_name() -> ObsString {
        obs_string!("PNGTooba")
    }
}

impl GetWidthSource for PngTuberSource {
    fn get_width(&mut self) -> u32 {
        self.width
    }
}

impl GetHeightSource for PngTuberSource {
    fn get_height(&mut self) -> u32 {
        self.height
    }
}

impl GetDefaultsSource for PngTuberSource {
    fn get_defaults(settings: &mut DataObj) {
        settings.set_default::<i64>(obs_string!("width"), 512);
        settings.set_default::<i64>(obs_string!("height"), 512);
        settings.set_default::<Cow<str>>(obs_string!("idle_image"), "");
        settings.set_default::<bool>(obs_string!("keep_idle_visible"), false);
        settings.set_default::<Cow<str>>(obs_string!("speaking_image"), "");
        settings.set_default::<Cow<str>>(obs_string!("mic_source"), "");
        settings.set_default::<f64>(obs_string!("mic_sensitivity"), 0.02);
        settings.set_default::<bool>(obs_string!("vowel_enabled"), false);
        settings.set_default::<f64>(obs_string!("vowel_smoothing"), 0.5);
        for vowel in Vowel::ALL {
            settings.set_default::<Cow<str>>(ObsString::from(vowel.settings_key()), "");
        }

        settings.set_default::<Cow<str>>(obs_string!("overlay_image"), "");
        settings.set_default::<Cow<str>>(obs_string!("when_speaking_effect"), EFFECT_NONE);
        settings.set_default::<f64>(obs_string!("shake_intensity"), 8.0);
        settings.set_default::<f64>(obs_string!("shake_speed"), 10.0);
        settings.set_default::<f64>(obs_string!("bounce_height"), 25.0);
        settings.set_default::<f64>(obs_string!("bounce_speed"), 2.5);
    }
}

impl UpdateSource for PngTuberSource {
    fn update(&mut self, settings: &mut DataObj, context: &mut GlobalContext) {
        self.apply_settings(settings, context);
    }
}

unsafe extern "C" fn effect_modified(
    props: *mut obs_properties_t,
    _property: *mut obs_property_t,
    settings: *mut obs_data_t,
) -> bool {
    let wrapped = DataObj::from_raw(settings);
    let effect = wrapped
        .get::<Cow<str>>("when_speaking_effect")
        .map(|s| s.into_owned())
        .unwrap_or_default();
    std::mem::forget(wrapped);

    set_visible_raw(props, "shake_intensity", effect == EFFECT_SHAKE);
    set_visible_raw(props, "shake_speed", effect == EFFECT_SHAKE);
    set_visible_raw(props, "bounce_height", effect == EFFECT_BOUNCE);
    set_visible_raw(props, "bounce_speed", effect == EFFECT_BOUNCE);
    true
}

impl GetPropertiesSource for PngTuberSource {
    fn get_properties(&mut self) -> Properties {
        let mut props = Properties::new();

        add_group(
            &mut props,
            obs_string!("general"),
            obs_string!("General"),
            |g| {
                g.add(obs_string!("width"), obs_string!("Width"), dimension_prop());
                g.add(
                    obs_string!("height"),
                    obs_string!("Height"),
                    dimension_prop(),
                );

                let mut mic_list = g.add_list::<ObsString>(
                    obs_string!("mic_source"),
                    obs_string!("Microphone"),
                    false,
                );
                mic_list.push(obs_string!("(None)"), obs_string!(""));
                for name in list_audio_sources() {
                    mic_list.push(ObsString::from(name.clone()), ObsString::from(name));
                }

                g.add(
                    obs_string!("mic_sensitivity"),
                    obs_string!("Mic Sensitivity"),
                    NumberProp::<f64>::new_float(0.001)
                        .with_range(0.0..=1.0)
                        .with_slider(),
                );
            },
        );

        add_group(
            &mut props,
            obs_string!("idle_speaking"),
            obs_string!("Idle / Speaking Images"),
            |g| {
                add_image_field(g, "idle_image", obs_string!("Idle Image"), clear_idle_image);
                g.add(
                    obs_string!("keep_idle_visible"),
                    obs_string!("Keep Idle Image Visible While Talking"),
                    BoolProp,
                );
                add_image_field(
                    g,
                    "speaking_image",
                    obs_string!("Speaking Image"),
                    clear_speaking_image,
                );
            },
        );

        add_group(
            &mut props,
            obs_string!("vowels"),
            obs_string!("Vowel Mouth Shapes"),
            |g| {
                g.add(
                    obs_string!("vowel_enabled"),
                    obs_string!("Enable Vowel Mouth Shapes"),
                    BoolProp,
                );
                g.add(
                    obs_string!("vowel_smoothing"),
                    obs_string!("Vowel Smoothing"),
                    NumberProp::<f64>::new_float(0.01)
                        .with_range(0.0..=0.95)
                        .with_slider(),
                );
                for vowel in Vowel::ALL {
                    let clear_callback = match vowel {
                        Vowel::A => clear_vowel_a_image,
                        Vowel::E => clear_vowel_e_image,
                        Vowel::I => clear_vowel_i_image,
                        Vowel::O => clear_vowel_o_image,
                        Vowel::U => clear_vowel_u_image,
                    };
                    add_image_field(
                        g,
                        vowel.settings_key(),
                        ObsString::from(vowel.label()),
                        clear_callback,
                    );
                }
            },
        );

        add_group(
            &mut props,
            obs_string!("effects"),
            obs_string!("Effects"),
            |g| {
                add_image_field(
                    g,
                    "overlay_image",
                    obs_string!("Overlay Image (shown on top of everything else)"),
                    clear_overlay_image,
                );

                let mut effect_list = g.add_list::<ObsString>(
                    obs_string!("when_speaking_effect"),
                    obs_string!("When Speaking"),
                    false,
                );
                effect_list.push(obs_string!("None"), ObsString::from(EFFECT_NONE));
                effect_list.push(obs_string!("Shake"), ObsString::from(EFFECT_SHAKE));
                effect_list.push(obs_string!("Bounce"), ObsString::from(EFFECT_BOUNCE));
                unsafe {
                    obs_property_set_modified_callback(
                        effect_list.as_ptr() as *mut _,
                        Some(effect_modified),
                    );
                }

                g.add(
                    obs_string!("shake_intensity"),
                    obs_string!("Shake Intensity"),
                    NumberProp::<f64>::new_float(1.0)
                        .with_range(0.0..=100.0)
                        .with_slider(),
                );
                g.add(
                    obs_string!("shake_speed"),
                    obs_string!("Shake Speed"),
                    NumberProp::<f64>::new_float(0.5)
                        .with_range(0.0..=30.0)
                        .with_slider(),
                );
                g.add(
                    obs_string!("bounce_height"),
                    obs_string!("Bounce Height"),
                    NumberProp::<f64>::new_float(1.0)
                        .with_range(0.0..=300.0)
                        .with_slider(),
                );
                g.add(
                    obs_string!("bounce_speed"),
                    obs_string!("Bounce Speed"),
                    NumberProp::<f64>::new_float(0.1)
                        .with_range(0.0..=10.0)
                        .with_slider(),
                );

                set_visible(
                    g,
                    "shake_intensity",
                    self.when_speaking_effect == EFFECT_SHAKE,
                );
                set_visible(g, "shake_speed", self.when_speaking_effect == EFFECT_SHAKE);
                set_visible(
                    g,
                    "bounce_height",
                    self.when_speaking_effect == EFFECT_BOUNCE,
                );
                set_visible(
                    g,
                    "bounce_speed",
                    self.when_speaking_effect == EFFECT_BOUNCE,
                );
            },
        );

        props
    }
}

impl PngTuberSource {
    fn talking_asset(&self, vowel: Option<Vowel>, vowel_enabled: bool) -> Option<&ImageAsset> {
        if vowel_enabled {
            vowel
                .and_then(|v| self.vowel_assets[v.index()].as_ref())
                .or(self.speaking_asset.as_ref())
        } else {
            self.speaking_asset.as_ref()
        }
    }

    fn effect_offset(&self, talking: bool, elapsed: Duration) -> (i32, i32) {
        if !talking {
            return (0, 0);
        }

        let t = elapsed.as_secs_f32();
        match self.when_speaking_effect.as_str() {
            EFFECT_SHAKE => {
                let x = (t * self.shake_speed * 13.0).sin() * self.shake_intensity;
                let y = (t * self.shake_speed * 17.0).cos() * self.shake_intensity;
                (x as i32, y as i32)
            }
            EFFECT_BOUNCE => {
                let y = -(t * self.bounce_speed).sin().abs() * self.bounce_height;
                (0, y as i32)
            }
            _ => (0, 0),
        }
    }
}

impl VideoRenderSource for PngTuberSource {
    fn video_render(&mut self, _context: &mut GlobalContext, _render: &mut VideoRenderContext) {
        let (talking, vowel, vowel_enabled) = {
            let state = self.audio_state.lock().unwrap();
            (
                state.level > state.threshold,
                state.vowel,
                state.vowel_enabled,
            )
        };

        let elapsed = self.animation_start.elapsed();
        let draw_at = |asset: &ImageAsset, x: i32, y: i32| {
            asset
                .current_texture(elapsed)
                .draw(x, y, self.width, self.height, false)
        };
        let (offset_x, offset_y) = self.effect_offset(talking, elapsed);

        if self.keep_idle_visible {
            if let Some(idle) = self.idle_asset.as_ref() {
                draw_at(idle, offset_x, offset_y);
            }
            if talking {
                if let Some(overlay) = self.talking_asset(vowel, vowel_enabled) {
                    draw_at(overlay, offset_x, offset_y);
                }
            }
        } else {
            let asset = if talking {
                self.talking_asset(vowel, vowel_enabled)
            } else {
                self.idle_asset.as_ref()
            };
            if let Some(asset) = asset {
                draw_at(asset, offset_x, offset_y);
            }
        }

        if let Some(overlay) = self.overlay_asset.as_ref() {
            draw_at(overlay, offset_x, offset_y);
        }
    }
}
