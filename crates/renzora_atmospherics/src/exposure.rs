//! The camera's exposure, owned by the weatherscape root: the pipeline's night grade drives
//! Bevy's auto-exposure on every camera the pipeline is installed on.

use bevy::camera::Exposure;
use bevy::math::cubic_splines::LinearSpline;
use bevy::post_process::auto_exposure::{AutoExposure, AutoExposureCompensationCurve};
use bevy::prelude::*;
use bevy_atmospherics::{CloudReconstruction, NightGrade};

use crate::Weatherscape;

/// Puts the grade's exposure on a camera the pipeline was just installed on, and rebuilds it on
/// every camera when the authored grade moves. Only those two edges write: the host strips the
/// component itself on its lowest quality tier, and a writer that re-inserted on every frame would
/// fight it forever.
pub(crate) fn sync(
    mut commands: Commands,
    mut curves: ResMut<Assets<AutoExposureCompensationCurve>>,
    grades: Query<Ref<NightGrade>, With<Weatherscape>>,
    cameras: Query<Entity, With<CloudReconstruction>>,
    mut seen: Local<Vec<Entity>>,
) {
    let Some(grade) = grades.iter().next() else {
        seen.clear();
        return;
    };
    let moved = grade.is_changed();
    let mut kept = Vec::with_capacity(cameras.iter().len());
    for camera in &cameras {
        kept.push(camera);
        let fresh = !seen.contains(&camera);
        if !(fresh || moved) {
            continue;
        }
        let curve = AutoExposureCompensationCurve::from_curve(LinearSpline::new(grade.knots()))
            .expect("the grade's knots are monotonic in x by construction");
        commands.entity(camera).insert((
            // The physical base the histogram meters against.
            Exposure::SUNLIGHT,
            AutoExposure {
                range: grade.range(),
                compensation_curve: curves.add(curve),
                ..default()
            },
        ));
    }
    *seen = kept;
}
