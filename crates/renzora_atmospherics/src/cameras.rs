//! Which cameras receive the pipeline, and what it puts on them.

use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;
use bevy_atmospherics::{CloudReconstruction, SkyProbe, VolumetricClouds};
use renzora::core::{EffectRouting, PrimaryViewportCamera, ViewportCamera};

use crate::Weatherscape;

/// The cameras a scene entity is routed to.
pub type Targets<'w, 's> =
    Query<'w, 's, (Has<PrimaryViewportCamera>, Has<ViewportCamera>), With<Camera3d>>;

/// The cameras routed from `source` that take a scene-wide effect: the primary viewport camera and
/// the runtime camera. The other three editor viewports and every panel camera carry no
/// atmosphere, and a second atmosphere-bearing camera crashes wgpu (bevy_atmospherics
/// `docs/spec/55-renzora-render-ownership.md`, "Which cameras receive the pipeline").
pub fn targets<'a>(
    routing: &'a EffectRouting,
    source: Entity,
    cameras: &'a Targets,
) -> impl Iterator<Item = Entity> + 'a {
    routing
        .iter()
        .filter(move |(_, sources)| sources.contains(&source))
        .filter_map(move |(target, _)| {
            let (primary, viewport) = cameras.get(*target).ok()?;
            (primary || !viewport).then_some(*target)
        })
}

/// Installs the per-camera half of the pipeline on every camera routed to the weatherscape root.
pub(crate) fn install(
    mut commands: Commands,
    routing: Res<EffectRouting>,
    roots: Query<Entity, With<Weatherscape>>,
    cameras: Targets,
    installed: Query<(), With<CloudReconstruction>>,
) {
    let Some(root) = roots.iter().next() else {
        return;
    };
    for target in targets(&routing, root, &cameras) {
        if installed.contains(target) {
            continue;
        }
        commands.entity(target).insert((
            VolumetricClouds::default(),
            CloudReconstruction::default(),
            SkyProbe::default(),
            // The froxel volume and the cloud composite both read it.
            DepthPrepass,
            // A multisampled depth view does not bind to the trace's non-multisampled binding.
            Msaa::Off,
        ));
    }
}
