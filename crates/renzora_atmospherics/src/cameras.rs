//! Which cameras receive the pipeline, and what it puts on them.

use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::post_process::auto_exposure::AutoExposure;
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

/// Keeps the per-camera half of the pipeline on exactly the cameras routed to a weatherscape root.
/// The root going, by a delete or the host's scene sweep, takes the pipeline off every camera it
/// was on: the adapter owns the whole bundle and resets what it installed when the root goes
/// (bevy_atmospherics `docs/spec/55-renzora-render-ownership.md`, "Scene switching, play, and
/// teardown").
pub fn sync(
    mut commands: Commands,
    routing: Res<EffectRouting>,
    roots: Query<Entity, With<Weatherscape>>,
    cameras: Targets,
    installed: Query<Entity, With<CloudReconstruction>>,
) {
    let wanted: Vec<Entity> = roots
        .iter()
        .next()
        .map(|root| targets(&routing, root, &cameras).collect())
        .unwrap_or_default();
    for camera in &installed {
        if !wanted.contains(&camera) {
            commands.entity(camera).remove::<(
                VolumetricClouds,
                CloudReconstruction,
                SkyProbe,
                AutoExposure,
            )>();
        }
    }
    for target in wanted {
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
