//! The per-camera half of the pipeline follows the weatherscape root: on the routed camera while
//! the root lives, off it once the root goes.

use bevy::post_process::auto_exposure::AutoExposure;
use bevy::prelude::*;
use bevy_atmospherics::{CloudReconstruction, SkyProbe, VolumetricClouds};
use renzora::core::{EffectRouting, PrimaryViewportCamera, ViewportCamera};
use renzora_atmospherics::{BauerPackage, cameras, weatherscape_bundle};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<EffectRouting>()
        .add_systems(Update, cameras::sync);
    app
}

fn installed(world: &mut World, camera: Entity) -> bool {
    let entity = world.entity(camera);
    entity.contains::<VolumetricClouds>()
        && entity.contains::<CloudReconstruction>()
        && entity.contains::<SkyProbe>()
}

#[test]
fn the_pipeline_leaves_the_camera_with_the_root() {
    let mut app = app();
    let primary = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            PrimaryViewportCamera,
            ViewportCamera(0),
        ))
        .id();
    let secondary = app
        .world_mut()
        .spawn((Camera3d::default(), ViewportCamera(1)))
        .id();
    let root = app
        .world_mut()
        .spawn(weatherscape_bundle(1, BauerPackage::default()))
        .id();
    app.world_mut().resource_mut::<EffectRouting>().routes =
        vec![(primary, vec![root]), (secondary, vec![root])];
    app.update();
    assert!(installed(app.world_mut(), primary));
    assert!(
        !installed(app.world_mut(), secondary),
        "a secondary editor viewport takes no atmosphere"
    );

    // The root authors the view; the camera carries a copy that follows it.
    app.world_mut()
        .entity_mut(root)
        .get_mut::<VolumetricClouds>()
        .unwrap()
        .max_steps = 96;
    app.update();
    assert_eq!(
        app.world().entity(primary).get::<VolumetricClouds>().unwrap().max_steps,
        96
    );

    // The exposure half lands from its own system; the root's despawn takes it too.
    app.world_mut()
        .entity_mut(primary)
        .insert(AutoExposure::default());
    app.world_mut().entity_mut(root).despawn();
    app.world_mut().resource_mut::<EffectRouting>().routes = vec![];
    app.update();
    let camera = app.world().entity(primary);
    assert!(!camera.contains::<VolumetricClouds>());
    assert!(!camera.contains::<CloudReconstruction>());
    assert!(!camera.contains::<SkyProbe>());
    assert!(!camera.contains::<AutoExposure>());
}
