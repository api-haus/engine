//! The authored Bauer package reference, and the publication derived from it.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_atmospherics::BauerField;
use bevy_atmospherics::bauer::{Event, FieldRuntime, U64Hex, load};

/// A package is a path plus its digest, never a `Handle`: a handle fails the host's reflected RON
/// round trip and is dropped with no diagnostic (bevy_atmospherics
/// `docs/spec/55-renzora-persistence.md`, "What a scene save writes").
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct BauerPackage {
    /// Project-relative: the host's asset root is the project directory itself, with no `assets/`
    /// level under it.
    pub path: String,
    /// Local civil hour the field is accepted at.
    pub hour: f64,
}

impl Default for BauerPackage {
    fn default() -> Self {
        Self {
            path: "bauer/cumulonimbus-candidate".into(),
            hour: 10.0,
        }
    }
}

/// What the last accepted publication was built from, so a root whose package did not change is
/// not reloaded and re-uploaded every frame.
#[derive(Resource, Default)]
pub(crate) struct Accepted(Option<BauerPackage>);

/// Loads the authored package and publishes it. A package that fails to load leaves the host
/// running with the field unavailable and the failure reported, never a stopped startup.
pub(crate) fn publish(
    mut commands: Commands,
    mut accepted: ResMut<Accepted>,
    roots: Query<&BauerPackage, With<crate::Weatherscape>>,
    project: Option<Res<renzora::CurrentProject>>,
) {
    let authored = roots.iter().next().cloned();
    if authored == accepted.0 {
        return;
    }
    accepted.0 = authored.clone();
    let Some(authored) = authored else {
        commands.insert_resource(BauerField::default());
        return;
    };
    let root = match project {
        Some(project) => project.path.join(&authored.path),
        None => std::path::PathBuf::from(&authored.path),
    };
    let package = match load(&root) {
        Ok(package) => Arc::new(package),
        Err(e) => {
            error!("bauer package {}: {e}", root.display());
            commands.insert_resource(BauerField::default());
            return;
        }
    };
    let mut runtime = FieldRuntime::new();
    runtime.submit(Event::Load {
        request_id: U64Hex(1),
        manifest: "manifest.json".into(),
        sha256: package.manifest_sha256,
        hour: authored.hour,
    });
    runtime.produced(U64Hex(1), Some(package.clone()));
    commands.insert_resource(BauerField {
        snapshot: runtime.accepted().clone(),
        package: Some(package),
        parameters: runtime.parameters().cloned(),
        render_origin_m: runtime.render_origin_m(),
    });
}
