//! The weatherscape root. Every value is a component rather than a resource: the host's scene
//! format serializes no reflected resource at all (bevy_atmospherics
//! `docs/spec/55-renzora-persistence.md`, "Resources do not persist").

use bevy::light::DirectionalLight;
use bevy::prelude::*;
use bevy_atmospherics::{
    CelestialSettings, CloudLayer, CloudReconstruction, CloudShadows, Fog, LightRays, Location,
    MoonLight, NightGrade, Rainbow, SkyElements, SunClock, SunLight, VolumetricClouds, Weather,
    WeatherParticles, celestial_bundle,
};
use renzora_atmosphere::AtmosphereComponentSettings;

use crate::package::BauerPackage;

/// Marks the root and carries its identity. The host mints no stable id: load allocates fresh
/// entities and remaps only entity-typed fields, so a record the editor references across a
/// round trip is this field and not a `Entity`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Default, Reflect)]
#[reflect(Component)]
pub struct Weatherscape {
    pub id: u64,
}

/// The whole authored surface, as one entity with the two celestial lights under it. The view and
/// the reconstruction are authored here and copied onto every camera the pipeline installs on.
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
        (
            VolumetricClouds::default(),
            CloudReconstruction::default(),
            CloudLayer::default(),
            CloudShadows::default(),
            Fog::default(),
            LightRays::default(),
            Rainbow::default(),
            WeatherParticles::default(),
            Weather::default(),
            SkyElements::default(),
            NightGrade::default(),
        ),
        // The host draws its sky only from a routed entity carrying its own atmosphere source, and
        // the root is that entity: the planet, the medium and the per-camera method stay the
        // host's (bevy_atmospherics `docs/spec/55-renzora-render-ownership.md`).
        AtmosphereComponentSettings::default(),
        celestial_bundle(
            Location::default(),
            SunClock::default(),
            CelestialSettings::default(),
            true,
        ),
    )
}

type Present = (
    Has<BauerPackage>,
    Has<NightGrade>,
    Has<AtmosphereComponentSettings>,
    Has<VolumetricClouds>,
    Has<CloudReconstruction>,
);

/// A light under a root that lost its marker. The sun's marker did not reflect as a component
/// until 2026-09-07, so a file saved before then carries the light and not the marker, and the
/// celestial drive has nothing to write.
type Unmarked<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Name, &'static ChildOf),
    (
        With<DirectionalLight>,
        Without<SunLight>,
        Without<MoonLight>,
    ),
>;

/// A root authored before a component joined the bundle takes that component's default, so a
/// scene file from an earlier bundle still carries the whole surface once it loads.
pub(crate) fn complete(
    mut commands: Commands,
    roots: Query<(Entity, Present), With<Weatherscape>>,
    lights: Unmarked,
) {
    for (root, (package, grade, atmosphere, view, reconstruction)) in &roots {
        // A file written when the package carried an hour of its own no longer decodes, and the
        // host drops the record with one warning.
        if !package {
            commands.entity(root).insert(BauerPackage::default());
        }
        if !grade {
            commands.entity(root).insert(NightGrade::default());
        }
        if !atmosphere {
            commands
                .entity(root)
                .insert(AtmosphereComponentSettings::default());
        }
        if !view {
            commands.entity(root).insert(VolumetricClouds::default());
        }
        if !reconstruction {
            commands.entity(root).insert(CloudReconstruction::default());
        }
    }
    for (light, name, parent) in &lights {
        if !roots.contains(parent.parent()) {
            continue;
        }
        if name.eq_ignore_ascii_case("sun") {
            commands.entity(light).insert(SunLight::default());
        } else if name.eq_ignore_ascii_case("moon") {
            commands.entity(light).insert(MoonLight);
        }
    }
}
