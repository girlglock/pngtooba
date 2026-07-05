use obs_wrapper::{
    obs_sys::{
        obs_group_type_OBS_GROUP_NORMAL, obs_properties_add_group, obs_properties_get,
        obs_properties_t, obs_property_set_visible,
    },
    properties::Properties,
    string::ObsString,
    wrapper::PtrWrapper,
};
use std::ffi::CString;

pub fn add_group(
    parent: &mut Properties,
    key: ObsString,
    label: ObsString,
    build: impl FnOnce(&mut Properties),
) {
    let mut group = Properties::new();
    build(&mut group);
    unsafe {
        obs_properties_add_group(
            parent.as_ptr_mut(),
            key.as_ptr(),
            label.as_ptr(),
            obs_group_type_OBS_GROUP_NORMAL,
            group.into_raw(),
        );
    }
}

pub fn set_visible(props: &mut Properties, key: &str, visible: bool) {
    unsafe { set_visible_raw(props.as_ptr_mut(), key, visible) };
}

pub unsafe fn set_visible_raw(props: *mut obs_properties_t, key: &str, visible: bool) {
    let Ok(name) = CString::new(key) else { return };
    let property = obs_properties_get(props, name.as_ptr());
    if !property.is_null() {
        obs_property_set_visible(property, visible);
    }
}
