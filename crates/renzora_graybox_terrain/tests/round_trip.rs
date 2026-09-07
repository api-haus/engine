//! The terrain root through the host's own scene save and load: the root and its transform
//! survive, and nothing the harness derives from it is in the file.

use bevy::prelude::*;
use renzora_engine::scene_io::{load_scene_from_string, serialize_scene_to_string};
use renzora_graybox_terrain::{graybox_terrain_bundle, GrayboxTerrain};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::transform::TransformPlugin)
        .register_type::<GrayboxTerrain>();
    app
}

#[test]
fn the_terrain_root_survives_a_save_and_reopen() {
    let mut before = app();
    let root = before
        .world_mut()
        .spawn(graybox_terrain_bundle())
        .insert(Transform::from_xyz(0.0, -3.0, 0.0))
        .id();
    // What the runtime builds under the root: a hidden holder, and named scenery under it.
    let holder = before
        .world_mut()
        .spawn((renzora::HideInHierarchy, ChildOf(root)))
        .id();
    before
        .world_mut()
        .spawn((Name::new("Sea level — metres"), ChildOf(holder)));
    before.update();
    let ron = serialize_scene_to_string(before.world_mut()).expect("the scene serializes");

    let mut after = app();
    load_scene_from_string(after.world_mut(), &ron);
    after.update();

    let world = after.world_mut();
    let mut roots = world.query::<(&GrayboxTerrain, &Transform, &Name)>();
    let (_, transform, name) = roots.single(world).expect("one terrain root");
    assert_eq!(name.as_str(), "Graybox Terrain");
    assert_eq!(transform.translation, Vec3::new(0.0, -3.0, 0.0));
    let mut named = world.query::<&Name>();
    assert_eq!(
        named.iter(world).count(),
        1,
        "the derived scene is not in the file"
    );
}
