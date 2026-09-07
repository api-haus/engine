//! Which cameras receive the pipeline, and what it puts on them.

use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;
use bevy_atmospherics::bauer::CloudModel;
use bevy_atmospherics::{CloudReconstruction, SkyProbe, SkyProbeAblation, VolumetricClouds};
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
    // Open on 2026-09-07: with the probe on the camera, despawning the weatherscape root hangs
    // the GPU (`NVRM: Xid 109 CTX SWITCH TIMEOUT`). `ATMOS_NO_PROBE=1` leaves it off;
    // `ATMOS_PROBE_ABLATE=cloud|sky|fog` skips one of its inputs for the bisect.
    let env = |name: &str| std::env::var(name).unwrap_or_default();
    let probe = env("ATMOS_NO_PROBE") != "1";
    let ablate = env("ATMOS_PROBE_ABLATE");
    commands.insert_resource(SkyProbeAblation {
        cloud_pass: ablate != "cloud",
        sky_pass: ablate != "sky",
    });
    for target in targets(&routing, root, &cameras) {
        if installed.contains(target) {
            continue;
        }
        commands.entity(target).insert((
            // The host renders Bauer only (bevy_atmospherics `docs/spec/54-graybox-terrain.md`,
            // "Running the scene"); the camera's default is the legacy noise model.
            VolumetricClouds {
                model: if env("ATMOS_LEGACY") == "1" {
                    CloudModel::Legacy
                } else {
                    CloudModel::Bauer
                },
                ..default()
            },
            CloudReconstruction::default(),
            // The froxel volume and the cloud composite both read it.
            DepthPrepass,
            // A multisampled depth view does not bind to the trace's non-multisampled binding.
            Msaa::Off,
        ));
        if probe {
            commands.entity(target).insert(SkyProbe {
                include_fog: ablate != "fog",
                ..default()
            });
        }
    }
}
