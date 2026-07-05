use obs_wrapper::obs_sys::{
    obs_enum_sources, obs_source_get_name, obs_source_get_output_flags, obs_source_t,
    OBS_SOURCE_AUDIO,
};
use std::ffi::{c_void, CStr};

unsafe extern "C" fn collect(param: *mut c_void, source: *mut obs_source_t) -> bool {
    let names = &mut *(param as *mut Vec<String>);
    if obs_source_get_output_flags(source) & OBS_SOURCE_AUDIO != 0 {
        let name_ptr = obs_source_get_name(source);
        if !name_ptr.is_null() {
            if let Ok(name) = CStr::from_ptr(name_ptr).to_str() {
                names.push(name.to_string());
            }
        }
    }
    true
}

pub fn list_audio_sources() -> Vec<String> {
    let mut names = Vec::new();
    unsafe {
        obs_enum_sources(Some(collect), &mut names as *mut Vec<String> as *mut c_void);
    }
    names
}
