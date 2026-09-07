//! Editor half of the atmospherics host adapter: the weatherscape root in Add Entity, and the
//! drawers for what it carries.

use bevy::prelude::*;
use bevy_atmospherics::Weather;
use renzora::{AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_atmospherics::{weatherscape_bundle, BauerPackage, Weatherscape};

#[derive(Default)]
pub struct AtmosphericsEditorPlugin;

impl Plugin for AtmosphericsEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AtmosphericsEditorPlugin");
        app.register_entity_preset(EntityPreset {
            id: "weatherscape",
            display_name: "Weatherscape",
            icon: "cloud",
            category: "general",
            spawn_fn: |world| {
                let id = next_id(world);
                world
                    .spawn(weatherscape_bundle(id, BauerPackage::default()))
                    .id()
            },
        })
        .register_inspector(package_entry())
        .register_inspector(weather_entry());
    }
}

/// The host mints no stable id, so one is minted here and carried in the scene file like any other
/// value (bevy_atmospherics `docs/spec/55-renzora-persistence.md`, "Identity").
fn next_id(world: &mut World) -> u64 {
    world
        .query::<&Weatherscape>()
        .iter(world)
        .map(|root| root.id)
        .max()
        .unwrap_or(0)
        + 1
}

fn package_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "bauer_package",
        display_name: "Bauer Package",
        icon: "cloud",
        category: "rendering",
        has_fn: |world, entity| world.get::<BauerPackage>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(BauerPackage::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<BauerPackage>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Package",
                field_type: FieldType::String,
                get_fn: |w, e| {
                    w.get::<BauerPackage>(e)
                        .map(|p| FieldValue::String(p.path.clone()))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::String(path), Some(mut p)) =
                        (v, w.get_mut::<BauerPackage>(e))
                    {
                        p.path = path;
                    }
                },
            },
            FieldDef {
                name: "Hour",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 24.0,
                },
                get_fn: |w, e| {
                    w.get::<BauerPackage>(e)
                        .map(|p| FieldValue::Float(p.hour as f32))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(hour), Some(mut p)) =
                        (v, w.get_mut::<BauerPackage>(e))
                    {
                        p.hour = hour as f64;
                    }
                },
            },
        ],
    }
}

fn weather_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "weather",
        display_name: "Weather",
        icon: "drop",
        category: "rendering",
        has_fn: |world, entity| world.get::<Weather>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Weather::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Weather>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            renzora::float_field!("Rain", Weather, rain, 0.01, 0.0, 1.0),
            renzora::float_field!("Snow", Weather, snow, 0.01, 0.0, 1.0),
            renzora::float_field!("Fog", Weather, fog, 0.01, 0.0, 1.0),
            renzora::float_field!("Thunder", Weather, thunder, 0.01, 0.0, 1.0),
            renzora::float_field!("Wind", Weather, wind_intensity, 0.01, 0.0, 1.0),
        ],
    }
}

renzora::add!(AtmosphericsEditorPlugin, Editor);
