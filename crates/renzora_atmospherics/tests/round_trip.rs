//! The scene round trip of the first integration proof, against the host's own save and load: a
//! component the save drops is dropped silently, so only the shipped path can prove it survived.

use bevy::prelude::*;
use bevy_atmospherics::{
    CelestialSettings, CloudLayer, CloudReconstruction, CloudShadows, Fog, LightRays, Location,
    MoonLight, NightGrade, Rainbow, SkyElements, SunClock, SunLight, VolumetricClouds, Weather,
    WeatherParticles,
};
use renzora_atmospherics::{
    BauerPackage, Weatherscape, register_authored_types, weatherscape_bundle,
};
use renzora_engine::scene_io::{load_scene_from_string, serialize_scene_to_string};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::transform::TransformPlugin);
    register_authored_types(&mut app);
    app
}

#[test]
fn the_authored_root_survives_a_save_and_reopen() {
    let mut before = app();
    // The editor's own preset, then the rows an author moved off their defaults, so an equality
    // that passes on a component the file never carried is not possible.
    before
        .world_mut()
        .spawn(weatherscape_bundle(
            7,
            BauerPackage {
                path: "bauer/authored-elsewhere".into(),
            },
        ))
        .insert((
            Transform::from_xyz(120.0, 0.0, -340.0),
            Weather {
                rain: 0.75,
                ..default()
            },
            SunClock {
                seconds: 16.5 * 3600.0,
                ..default()
            },
        ));
    before.update();
    let ron = serialize_scene_to_string(before.world_mut()).expect("the scene serializes");

    let mut after = app();
    load_scene_from_string(after.world_mut(), &ron);
    after.update();

    let world = after.world_mut();
    let mut roots = world.query::<(
        (&Weatherscape, &BauerPackage, &Transform, &Weather, &SunClock),
        (
            &CloudLayer,
            &CloudShadows,
            &Fog,
            &LightRays,
            &Rainbow,
            &WeatherParticles,
            &SkyElements,
            &Location,
            &CelestialSettings,
            &NightGrade,
            &VolumetricClouds,
            &CloudReconstruction,
        ),
    )>();
    let ((scape, package, transform, weather, clock), _) =
        roots.single(world).expect("one weatherscape root");

    assert_eq!(scape.id, 7);
    assert_eq!(package.path, "bauer/authored-elsewhere");
    assert_eq!(transform.translation, Vec3::new(120.0, 0.0, -340.0));
    assert_eq!(weather.rain, 0.75);
    assert_eq!(clock.seconds, 16.5 * 3600.0);

    // The two lights keep the markers the celestial drive selects them by.
    let world = after.world_mut();
    assert_eq!(world.query::<&SunLight>().iter(world).count(), 1);
    assert_eq!(world.query::<&MoonLight>().iter(world).count(), 1);
}
