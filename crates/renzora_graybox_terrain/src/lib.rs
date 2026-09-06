//! The Wellington graybox terrain as a Renzora scene input (bevy_atmospherics
//! `docs/spec/54-graybox-terrain.md`). The clipmap is not spawned yet: the harness grounds its own
//! camera and owns its lights, and both couplings are traced against the host before installation.

use bevy::prelude::*;

#[derive(Default)]
pub struct GrayboxTerrainPlugin;

impl Plugin for GrayboxTerrainPlugin {
    fn build(&self, _app: &mut App) {
        info!("[runtime] GrayboxTerrainPlugin");
    }
}

renzora::add!(GrayboxTerrainPlugin);
