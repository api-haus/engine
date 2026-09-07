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

/// The root and what it authors for the cameras.
type Roots<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<Ref<'static, VolumetricClouds>>,
        Option<Mut<'static, CloudReconstruction>>,
    ),
    With<Weatherscape>,
>;

/// Keeps the per-camera half of the pipeline on exactly the cameras routed to a weatherscape root.
/// The root going, by a delete or the host's scene sweep, takes the pipeline off every camera it
/// was on: the adapter owns the whole bundle and resets what it installed when the root goes
/// (bevy_atmospherics `docs/spec/55-renzora-render-ownership.md`, "Scene switching, play, and
/// teardown").
pub fn sync(
    mut commands: Commands,
    routing: Res<EffectRouting>,
    mut roots: Roots,
    cameras: Targets,
    // The root carries the authored copy of the same components; only a camera is an install.
    installed: Query<Entity, (With<CloudReconstruction>, With<Camera3d>)>,
) {
    let Some((root, view, mut reconstruction)) = roots.iter_mut().next() else {
        for camera in &installed {
            commands.entity(camera).remove::<(
                VolumetricClouds,
                CloudReconstruction,
                SkyProbe,
                AutoExposure,
            )>();
        }
        return;
    };
    let wanted: Vec<Entity> = targets(&routing, root, &cameras).collect();
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
    // The reset flag is handed to the cameras once and lowered on the root, so the file never
    // carries a raised one.
    let view_moved = view.as_ref().is_some_and(|view| view.is_changed());
    let reconstruction_moved = reconstruction
        .as_ref()
        .is_some_and(|reconstruction| reconstruction.is_changed());
    let authored_view = view.as_deref().copied().unwrap_or_default();
    let authored_reconstruction = reconstruction.as_deref().copied().unwrap_or_default();
    for target in wanted {
        let fresh = !installed.contains(target);
        if fresh {
            commands.entity(target).insert((
                SkyProbe::default(),
                // The froxel volume and the cloud composite both read it.
                DepthPrepass,
                // A multisampled depth view does not bind to the trace's non-multisampled binding.
                Msaa::Off,
            ));
        }
        if fresh || view_moved {
            commands.entity(target).insert(authored_view);
        }
        if fresh || reconstruction_moved {
            commands.entity(target).insert(authored_reconstruction);
        }
    }
    if let Some(reconstruction) = reconstruction.as_deref_mut() {
        if reconstruction.reset {
            reconstruction.reset = false;
        }
    }
}
