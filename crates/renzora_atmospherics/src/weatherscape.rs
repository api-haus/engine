//! The weatherscape root. Every value is a component rather than a resource: the host's scene
//! format serializes no reflected resource at all (bevy_atmospherics
//! `docs/spec/55-renzora-persistence.md`, "Resources do not persist").

use bevy::prelude::*;
use bevy_atmospherics::{
    CelestialSettings, CloudLayer, CloudShadows, Fog, LightRays, Location, Rainbow, SkyElements,
    SunClock, Weather, WeatherParticles, celestial_bundle,
};

use crate::package::BauerPackage;

/// Marks the root and carries its identity. The host mints no stable id: load allocates fresh
/// entities and remaps only entity-typed fields, so a record the editor references across a
/// round trip is this field and not a `Entity`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default, Reflect)]
#[reflect(Component)]
pub struct Weatherscape {
    pub id: u64,
}

/// The whole authored surface, as one entity with the two celestial lights under it.
///
/// One per scene. A scene carrying a second source of any of this is undefined and is not coded
/// for: both writers run and the last one wins per frame.
pub fn weatherscape_bundle(id: u64, package: BauerPackage) -> impl Bundle {
    (
        Name::new("Weatherscape"),
        Weatherscape { id },
        package,
        Transform::default(),
        Visibility::default(),
        CloudLayer::default(),
        CloudShadows::default(),
        Fog::default(),
        LightRays::default(),
        Rainbow::default(),
        WeatherParticles::default(),
        Weather::default(),
        SkyElements::default(),
        celestial_bundle(
            Location::default(),
            SunClock::default(),
            CelestialSettings::default(),
            true,
        ),
    )
}
