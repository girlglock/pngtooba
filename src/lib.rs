mod audio;
mod enumerate;
mod image_asset;
mod properties_ext;
mod source;
mod vowel;

use obs_wrapper::{obs_register_module, obs_string, prelude::*, source::SourceInfoBuilder};
use source::PngTuberSource;

struct PngToobaModule {
    context: ModuleContext,
}

impl Module for PngToobaModule {
    fn new(context: ModuleContext) -> Self {
        Self { context }
    }

    fn get_ctx(&self) -> &ModuleContext {
        &self.context
    }

    fn load(&mut self, load_context: &mut LoadContext) -> bool {
        let source_info: SourceInfoBuilder<PngTuberSource> =
            load_context.create_source_builder::<PngTuberSource>();

        let source = source_info
            .enable_get_name()
            .enable_get_width()
            .enable_get_height()
            .enable_get_defaults()
            .enable_get_properties()
            .enable_update()
            .enable_video_render()
            .build();

        load_context.register_source(source);
        true
    }

    fn description() -> ObsString {
        obs_string!("PNGTuber with support for vowels :3")
    }

    fn name() -> ObsString {
        obs_string!("PNGTooba")
    }

    fn author() -> ObsString {
        obs_string!("girlglock")
    }
}

obs_register_module!(PngToobaModule);
