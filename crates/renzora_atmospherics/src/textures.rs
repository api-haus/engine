//! The pipeline's textures against a project that opens after startup.

use std::path::PathBuf;

use bevy::asset::{AssetPath, LoadState};
use bevy::prelude::*;
use renzora_engine::ProjectAssetPath;

/// Every texture the pipeline requested at startup.
#[derive(Resource)]
pub(crate) struct Textures(pub Vec<String>);

/// The pipeline requests its textures at startup and the editor sets the asset root when a project
/// is picked, seconds later, so the startup request of every one of them fails. The root moving
/// reissues the failed ones on the handles the pipeline already holds. The reader's own root is
/// watched, not `CurrentProject`: the host copies one into the other in a system of its own, and a
/// reload issued the frame the project changed still ran against the old root.
pub(crate) fn reload(
    textures: Res<Textures>,
    root: Option<Res<ProjectAssetPath>>,
    server: Res<AssetServer>,
    mut seen: Local<Option<PathBuf>>,
) {
    let current = root.and_then(|root| root.0.read().ok()?.clone());
    if current.is_none() || current == *seen {
        return;
    }
    *seen = current;
    for path in &textures.0 {
        let path = AssetPath::parse(path);
        let failed = server
            .get_path_id(&path)
            .and_then(|id| server.get_load_state(id))
            .is_some_and(|state| matches!(state, LoadState::Failed(_)));
        if failed {
            server.reload(path);
        }
    }
}
