//! Runtime half of the atmospherics host adapter. The rendering pipeline is not installed yet:
//! render ownership and persistence are traced first (bevy_atmospherics
//! `docs/spec/55-renzora-integration.md`, "Render ownership before installation").

use bevy::prelude::*;

#[derive(Default)]
pub struct AtmosphericsPlugin;

impl Plugin for AtmosphericsPlugin {
    fn build(&self, _app: &mut App) {
        info!("[runtime] AtmosphericsPlugin");
    }
}

renzora::add!(AtmosphericsPlugin);
