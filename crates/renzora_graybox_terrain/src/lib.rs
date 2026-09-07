//! The Wellington graybox terrain as a Renzora scene input (bevy_atmospherics
//! `docs/spec/54-graybox-terrain.md`): one authored root, and under it the clipmap, the sea, the
//! synthetic massing and the scale references the harness builds.

use bevy::prelude::*;
use bevy_atmospherics_harness::terrain::graybox::{GrayboxPlugin, GrayboxRequest};
use renzora::{EffectRouting, HideInHierarchy};
use renzora_atmospherics::cameras::{targets, Targets};

/// Marks the authored root. The scene file carries this and the root's transform and nothing
/// else: everything the harness spawns is derived, hidden from the hierarchy, and rebuilt on load.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct GrayboxTerrain;

/// The derived scene under a root, so a root is built once.
#[derive(Component)]
struct Built(#[allow(dead_code)] Entity);

/// The whole authored surface. One per scene.
pub fn graybox_terrain_bundle() -> impl Bundle {
    (
        Name::new("Graybox Terrain"),
        GrayboxTerrain,
        Transform::default(),
        Visibility::default(),
    )
}

#[derive(Default)]
pub struct GrayboxTerrainPlugin;

impl Plugin for GrayboxTerrainPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GrayboxTerrainPlugin");
        app.add_plugins(GrayboxPlugin)
            .register_type::<GrayboxTerrain>()
            .add_systems(Update, build);
    }
}

/// Builds the harness scene under every root that has none, following the camera the root is
/// routed to. The derived entities carry names the host's save would otherwise gather, so they
/// hang under a hidden holder and the file keeps the root alone.
fn build(
    mut commands: Commands,
    routing: Res<EffectRouting>,
    roots: Query<Entity, (With<GrayboxTerrain>, Without<Built>)>,
    cameras: Targets,
) {
    for root in &roots {
        let Some(target) = targets(&routing, root, &cameras).next() else {
            continue;
        };
        let holder = commands
            .spawn((
                HideInHierarchy,
                ChildOf(root),
                GrayboxRequest {
                    target: Some(target),
                },
            ))
            .id();
        commands.entity(root).insert(Built(holder));
    }
}

renzora::add!(GrayboxTerrainPlugin);
