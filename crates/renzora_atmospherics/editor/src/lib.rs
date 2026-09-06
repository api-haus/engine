//! Editor half of the atmospherics host adapter. The weatherscape editor
//! (bevy_atmospherics `docs/spec/55-weatherscape-editor.md`) registers its entities, inspectors
//! and tools here.

use bevy::prelude::*;

#[derive(Default)]
pub struct AtmosphericsEditorPlugin;

impl Plugin for AtmosphericsEditorPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] AtmosphericsEditorPlugin");
    }
}

renzora::add!(AtmosphericsEditorPlugin, Editor);
