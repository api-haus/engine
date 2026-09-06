//! Which cameras receive the pipeline, and what it puts on them.

use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;
use bevy_atmospherics::{CloudReconstruction, SkyProbe, VolumetricClouds};
use renzora::core::{EffectRouting, PrimaryViewportCamera, ViewportCamera};

use crate::Weatherscape;

/// Installs the per-camera half of the pipeline on every camera routed to the weatherscape root.
///
/// Only the primary viewport camera and the runtime camera take it: the other three editor
/// viewports and every panel camera carry no atmosphere, and a second atmosphere-bearing camera
/// crashes wgpu (bevy_atmospherics `docs/spec/55-renzora-render-ownership.md`, "Which cameras
/// receive the pipeline").
pub(crate) fn install(
    mut commands: Commands,
    routing: Res<EffectRouting>,
    roots: Query<Entity, With<Weatherscape>>,
    targets: Query<(Has<PrimaryViewportCamera>, Has<ViewportCamera>), With<Camera3d>>,
    installed: Query<(), With<VolumetricClouds>>,
) {
    let Some(root) = roots.iter().next() else {
        return;
    };
    for (target, sources) in routing.iter() {
        if !sources.contains(&root) || installed.contains(*target) {
            continue;
        }
        let Ok((primary, viewport)) = targets.get(*target) else {
            continue;
        };
        if viewport && !primary {
            continue;
        }
        commands.entity(*target).insert((
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
