use std::collections::HashMap;

use eframe::egui::{Context, TextureHandle, TextureOptions};

use crate::home::banner_art::decode_banner_image;
use crate::home::bundled_art::bundled_portrait;

const PORTRAIT_TEXTURE_NAME_PREFIX: &str = "portrait-";

/// The bundled images of characters, decoded and uploaded the first time a seat shows one.
#[derive(Default)]
pub struct PortraitArtStore {
    textures: HashMap<(&'static str, &'static str), Option<TextureHandle>>,
}

impl PortraitArtStore {
    pub fn texture(
        &mut self,
        ctx: &Context,
        game_id: &'static str,
        character_id: &'static str,
    ) -> Option<TextureHandle> {
        self.textures
            .entry((game_id, character_id))
            .or_insert_with(|| {
                let image = decode_banner_image(bundled_portrait(game_id, character_id)?)?;
                let name = format!("{PORTRAIT_TEXTURE_NAME_PREFIX}{game_id}-{character_id}");
                Some(ctx.load_texture(name, image, TextureOptions::LINEAR))
            })
            .clone()
    }
}
