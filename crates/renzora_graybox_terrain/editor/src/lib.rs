//! Editor half of the graybox terrain adapter: the terrain root in Add Entity.

use bevy::prelude::*;
use renzora::{AppEditorExt, EntityPreset};
use renzora_graybox_terrain::graybox_terrain_bundle;

#[derive(Default)]
pub struct GrayboxTerrainEditorPlugin;

impl Plugin for GrayboxTerrainEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GrayboxTerrainEditorPlugin");
        app.register_entity_preset(EntityPreset {
            id: "graybox_terrain",
            display_name: "Graybox Terrain",
            icon: "mountains",
            category: "general",
            spawn_fn: |world| world.spawn(graybox_terrain_bundle()).id(),
        });
    }
}

renzora::add!(GrayboxTerrainEditorPlugin, Editor);
