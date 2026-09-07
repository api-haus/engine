//! Editor half of the atmospherics host adapter: the weatherscape root in Add Entity, and the
//! drawers for what it carries.

use bevy::prelude::*;
use bevy_atmospherics::SunClock;
use renzora::{AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_atmospherics::{BauerPackage, Weatherscape, weatherscape_bundle};

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
        // Every other authored component is drawn by the host's reflection path, with the range
        // each field declares (bevy_atmospherics `docs/spec/55-renzora-integration.md`,
        // "Editor integration points").
        .register_inspector(package_entry())
        .register_inspector(clock_entry());
    }
}

/// The clock as a civil time: the component holds seconds since midnight, and the host's
/// reflected section would offer that raw and a running clock overwriting it (the editor holds
/// the clock still; the rate runs under play).
fn clock_entry() -> InspectorEntry {
    InspectorEntry {
        // The host hides its generated section for a type under the entry keyed by the type's own
        // lowercased name, and under nothing else.
        type_id: "sunclock",
        display_name: "Sun Clock",
        icon: "clock",
        category: "rendering",
        has_fn: |world, entity| world.get::<SunClock>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Hour",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 24.0,
                },
                get_fn: |w, e| {
                    w.get::<SunClock>(e)
                        .map(|c| FieldValue::Float((c.seconds / 3600.0) as f32))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(hour), Some(mut c)) = (v, w.get_mut::<SunClock>(e)) {
                        c.set_day_fraction(f64::from(hour) / 24.0);
                    }
                },
            },
            FieldDef {
                name: "Rate",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 3600.0,
                },
                get_fn: |w, e| {
                    w.get::<SunClock>(e)
                        .map(|c| FieldValue::Float(c.rate as f32))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(rate), Some(mut c)) = (v, w.get_mut::<SunClock>(e)) {
                        c.rate = f64::from(rate);
                    }
                },
            },
            renzora::int_field!("Year", SunClock, year, i32, 1.0, 1900.0, 2200.0),
            renzora::int_field!("Month", SunClock, month, u32, 1.0, 1.0, 12.0),
            renzora::int_field!("Day", SunClock, day, u32, 1.0, 1.0, 31.0),
        ],
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
        type_id: "bauerpackage",
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

renzora::add!(AtmosphericsEditorPlugin, Editor);
