//! Runtime half of the atmospherics host adapter: the rendering pipeline, the authored root it
//! reads, and the cameras it installs on.

use bevy::prelude::*;
use bevy_atmospherics::{
    CelestialPlugin, CelestialSettings, CloudLayer, CloudShadows, Fog, LightRays,
    Location, Rainbow, SkyElements, SunClock, Weather, WeatherParticles,
};

pub mod cameras;
pub mod package;
pub mod textures;
pub mod weatherscape;

pub use package::BauerPackage;
pub use weatherscape::{weatherscape_bundle, Weatherscape};

#[derive(Default)]
pub struct AtmosphericsPlugin;

impl Plugin for AtmosphericsPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AtmosphericsPlugin");
        let pipeline = bevy_atmospherics::AtmosphericsPlugin::default();
        app.insert_resource(textures::Textures(pipeline.texture_assets().into()))
            .add_plugins((
                pipeline,
                // The scene file authors the celestial bundle on the weatherscape root, so the plugin
                // takes no second one of its own.
                CelestialPlugin {
                    spawn: false,
                    ..default()
                },
            ))
            .init_resource::<package::Accepted>()
            .add_systems(
                Update,
                (package::publish, cameras::sync, textures::reload),
            );
        register_authored_types(app);
    }
}

/// Every component the weatherscape root persists. A component the registry does not carry fails
/// the host's reflected RON round trip and is dropped from the file with no diagnostic
/// (bevy_atmospherics `docs/spec/55-renzora-persistence.md`, "What a scene save writes"), so the
/// list is one thing the plugin and the round-trip battery share rather than two that drift.
pub fn register_authored_types(app: &mut App) {
    app.register_type::<Weatherscape>()
        .register_type::<BauerPackage>()
        .register_type::<CloudLayer>()
        .register_type::<CloudShadows>()
        .register_type::<Fog>()
        .register_type::<LightRays>()
        .register_type::<Rainbow>()
        .register_type::<WeatherParticles>()
        .register_type::<Weather>()
        .register_type::<SkyElements>()
        .register_type::<CelestialSettings>()
        .register_type::<Location>()
        .register_type::<SunClock>();
}

renzora::add!(AtmosphericsPlugin);
