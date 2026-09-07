//! The authored Bauer package reference, and the publication derived from it.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_atmospherics::bauer::{Event, FieldRuntime, Package, U64Hex, load};
use bevy_atmospherics::{BauerField, SunClock};

/// A package is a path plus its digest, never a `Handle`: a handle fails the host's reflected RON
/// round trip and is dropped with no diagnostic (bevy_atmospherics
/// `docs/spec/55-renzora-persistence.md`, "What a scene save writes").
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct BauerPackage {
    /// Project-relative: the host's asset root is the project directory itself, with no `assets/`
    /// level under it.
    pub path: String,
}

impl Default for BauerPackage {
    fn default() -> Self {
        Self {
            path: "bauer/cumulonimbus-candidate".into(),
        }
    }
}

/// The field's tracks are keyed by civil hour, and the hour is the sun clock's: one clock for the
/// sky and the weather map. Quantised so a running clock re-accepts the field a few times an hour,
/// not every frame.
const HOUR_STEP: f64 = 0.1;

/// What the last accepted publication was built from, so a root whose package and hour did not
/// change is not re-accepted and re-uploaded every frame.
#[derive(Resource, Default)]
pub(crate) struct Accepted {
    authored: Option<BauerPackage>,
    hour_steps: i64,
    package: Option<Arc<Package>>,
}

/// Loads the authored package and publishes it at the clock's hour. A package that fails to load
/// leaves the host running with the field unavailable and the failure reported, never a stopped
/// startup.
pub(crate) fn publish(
    mut commands: Commands,
    mut accepted: ResMut<Accepted>,
    roots: Query<(&BauerPackage, Option<&SunClock>), With<crate::Weatherscape>>,
    project: Option<Res<renzora::CurrentProject>>,
) {
    let Some((authored, clock)) = roots.iter().next() else {
        if accepted.authored.take().is_some() {
            accepted.package = None;
            commands.insert_resource(BauerField::default());
        }
        return;
    };
    let hour = clock.map_or(10.0, |clock| clock.seconds / 3600.0);
    let hour_steps = (hour / HOUR_STEP).floor() as i64;
    let same_package = accepted.authored.as_ref() == Some(authored);
    if same_package && accepted.hour_steps == hour_steps {
        return;
    }
    if !same_package {
        accepted.authored = Some(authored.clone());
        let root = match project {
            Some(project) => project.path.join(&authored.path),
            None => std::path::PathBuf::from(&authored.path),
        };
        accepted.package = match load(&root) {
            Ok(package) => Some(Arc::new(package)),
            Err(e) => {
                error!("bauer package {}: {e}", root.display());
                None
            }
        };
    }
    accepted.hour_steps = hour_steps;
    let Some(package) = accepted.package.clone() else {
        commands.insert_resource(BauerField::default());
        return;
    };
    let mut runtime = FieldRuntime::new();
    runtime.submit(Event::Load {
        request_id: U64Hex(1),
        manifest: "manifest.json".into(),
        sha256: package.manifest_sha256,
        hour: hour_steps as f64 * HOUR_STEP,
    });
    runtime.produced(U64Hex(1), Some(package.clone()));
    commands.insert_resource(BauerField {
        snapshot: runtime.accepted().clone(),
        package: Some(package),
        parameters: runtime.parameters().cloned(),
        render_origin_m: runtime.render_origin_m(),
    });
}
