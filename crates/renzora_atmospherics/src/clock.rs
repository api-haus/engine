//! The sun clock under the host's play modes: held still while the scene is edited, running at
//! its authored rate under play and simulation (bevy_atmospherics
//! `docs/spec/55-weatherscape-editor.md`, "Editing and publication").

use bevy::prelude::*;
use bevy_atmospherics::SunClockHold;
use renzora::core::{PlayModeState, PlayState};

pub(crate) fn hold(play: Option<Res<PlayModeState>>, mut hold: ResMut<SunClockHold>) {
    // A shipped game carries no play mode, and its clock runs.
    let editing = play.is_some_and(|play| play.state == PlayState::Editing);
    if hold.0 != editing {
        hold.0 = editing;
    }
}
