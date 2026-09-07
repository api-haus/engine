//! Editor half of the atmospherics host adapter: the weatherscape root in Add Entity, and the
//! cards for what the host's reflection path cannot draw from a raw field.

use bevy::prelude::*;
use bevy_atmospherics::surface::{PRESETS, QUALITIES};
use bevy_atmospherics::{
    sun_position, CloudReconstruction, Location, SunClock, VolumetricClouds, CITIES,
};
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
        // A card hides the host's generated section for the type its `type_id` names in
        // lowercase; every other component is drawn from reflection.
        .register_inspector(package_entry())
        .register_inspector(clock_entry())
        .register_inspector(location_entry())
        .register_inspector(quality_entry());
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
        fields: vec![FieldDef {
            name: "Package",
            field_type: FieldType::String,
            get_fn: |w, e| {
                w.get::<BauerPackage>(e)
                    .map(|p| FieldValue::String(p.path.clone()))
            },
            set_fn: |w, e, v| {
                if let (FieldValue::String(path), Some(mut p)) = (v, w.get_mut::<BauerPackage>(e)) {
                    p.path = path;
                }
            },
        }],
    }
}

/// The clock as a civil time: the component holds seconds since midnight, and the host's
/// reflected section would offer that raw and a running clock overwriting it (the editor holds
/// the clock still; the rate runs under play).
fn clock_entry() -> InspectorEntry {
    InspectorEntry {
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
                field_type: FieldType::Int {
                    min: 0.0,
                    max: 23.0,
                },
                get_fn: |w, e| {
                    w.get::<SunClock>(e)
                        .map(|c| FieldValue::Float(c.hour() as f32))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(hour), Some(mut c)) = (v, w.get_mut::<SunClock>(e)) {
                        c.set_hour(hour.round().clamp(0.0, 23.0) as u32);
                    }
                },
            },
            FieldDef {
                name: "Minute",
                field_type: FieldType::Int {
                    min: 0.0,
                    max: 59.0,
                },
                get_fn: |w, e| {
                    w.get::<SunClock>(e)
                        .map(|c| FieldValue::Float(c.minute() as f32))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Float(minute), Some(mut c)) = (v, w.get_mut::<SunClock>(e))
                    {
                        c.set_minute(minute.round().clamp(0.0, 59.0) as u32);
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

/// The city table's names, then the entry an edit off the table lands on.
static CITY_OPTIONS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
    CITIES
        .iter()
        .map(|city| city.name)
        .chain(std::iter::once("Custom"))
        .collect()
});

/// Where the scene stands, with the city presets the debug widget offers and the readouts the
/// position gives back.
fn location_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "location",
        display_name: "Location",
        icon: "map-pin",
        category: "rendering",
        has_fn: |world, entity| world.get::<Location>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "City",
                field_type: FieldType::Enum {
                    options: CITY_OPTIONS.as_slice(),
                },
                get_fn: |w, e| {
                    w.get::<Location>(e).map(|location| {
                        FieldValue::Enum(
                            location
                                .city()
                                .map_or("Custom", |city| city.name)
                                .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(name), Some(mut location)) =
                        (v, w.get_mut::<Location>(e))
                    {
                        if let Some(city) = CITIES.iter().find(|city| city.name == name) {
                            *location = Location::from(*city);
                        }
                    }
                },
            },
            renzora::float_field!("Latitude", Location, latitude, 0.1, -90.0, 90.0),
            renzora::float_field!("Longitude", Location, longitude, 0.1, -180.0, 180.0),
            renzora::float_field!("Time Zone", Location, time_zone, 0.5, -12.0, 14.0),
            renzora::bool_field!("Daylight Saving", Location, daylight_saving),
            readout("Sun Elevation", elevation),
            readout("Sun Azimuth", azimuth),
            readout("Sunrise", sunrise),
            readout("Sunset", sunset),
        ],
    }
}

/// A row that reads the sun's position and writes nothing.
fn readout(name: &'static str, get_fn: fn(&World, Entity) -> Option<FieldValue>) -> FieldDef {
    FieldDef {
        name,
        field_type: FieldType::ReadOnly,
        get_fn,
        set_fn: |_, _, _| {},
    }
}

fn position(world: &World, entity: Entity) -> Option<bevy_atmospherics::SunPosition> {
    let location = world.get::<Location>(entity)?;
    let clock = world.get::<SunClock>(entity)?;
    Some(sun_position(location, clock))
}

fn elevation(world: &World, entity: Entity) -> Option<FieldValue> {
    position(world, entity).map(|p| FieldValue::ReadOnly(format!("{:.1}°", p.corrected_elevation)))
}

fn azimuth(world: &World, entity: Entity) -> Option<FieldValue> {
    position(world, entity).map(|p| FieldValue::ReadOnly(format!("{:.1}°", p.azimuth)))
}

fn sunrise(world: &World, entity: Entity) -> Option<FieldValue> {
    position(world, entity).map(|p| FieldValue::ReadOnly(civil(p.sunrise)))
}

fn sunset(world: &World, entity: Entity) -> Option<FieldValue> {
    position(world, entity).map(|p| FieldValue::ReadOnly(civil(p.sunset)))
}

/// A day fraction as `HH:MM`, or the sun's absence.
fn civil(fraction: f64) -> String {
    if !(0.0..=1.0).contains(&fraction) {
        return "none".to_string();
    }
    let minutes = (fraction * 24.0 * 60.0).round() as u32;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

const QUALITY_OPTIONS: [&str; 6] = ["Potato", "Low", "Medium", "High", "Cinematic", "Custom"];
const PRESET_OPTIONS: [&str; 6] = [
    "Deck",
    "Deck Accumulated",
    "Sharp",
    "Direct",
    "Reference",
    "Custom",
];

/// The named points of the view and the reconstruction, over the raw rows the reflected sections
/// carry.
fn quality_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "cloud_quality",
        display_name: "Cloud Quality",
        icon: "sliders",
        category: "rendering",
        has_fn: |world, entity| {
            world.get::<VolumetricClouds>(entity).is_some()
                && world.get::<CloudReconstruction>(entity).is_some()
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Tier",
                field_type: FieldType::Enum {
                    options: &QUALITY_OPTIONS,
                },
                get_fn: |w, e| {
                    let view = w.get::<VolumetricClouds>(e)?;
                    let index = QUALITIES
                        .iter()
                        .position(|q| q.steps() == view.max_steps)
                        .unwrap_or(QUALITY_OPTIONS.len() - 1);
                    Some(FieldValue::Enum(QUALITY_OPTIONS[index].to_string()))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(label), Some(mut view)) =
                        (v, w.get_mut::<VolumetricClouds>(e))
                    {
                        if let Some(quality) = QUALITY_OPTIONS
                            .iter()
                            .position(|l| *l == label)
                            .and_then(|i| QUALITIES.get(i))
                        {
                            view.max_steps = quality.steps();
                        }
                    }
                },
            },
            FieldDef {
                name: "Reconstruction",
                field_type: FieldType::Enum {
                    options: &PRESET_OPTIONS,
                },
                get_fn: |w, e| {
                    let reconstruction = w.get::<CloudReconstruction>(e)?;
                    let grid = |r: &CloudReconstruction| {
                        (r.reconstruct_factor, r.trace_factor, r.accumulation_frames)
                    };
                    let index = PRESETS
                        .iter()
                        .position(|p| {
                            grid(&CloudReconstruction::preset(*p)) == grid(reconstruction)
                        })
                        .unwrap_or(PRESET_OPTIONS.len() - 1);
                    Some(FieldValue::Enum(PRESET_OPTIONS[index].to_string()))
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(label), Some(mut reconstruction)) =
                        (v, w.get_mut::<CloudReconstruction>(e))
                    {
                        if let Some(preset) = PRESET_OPTIONS
                            .iter()
                            .position(|l| *l == label)
                            .and_then(|i| PRESETS.get(i))
                        {
                            let point = CloudReconstruction::preset(*preset);
                            reconstruction.reconstruct_factor = point.reconstruct_factor;
                            reconstruction.trace_factor = point.trace_factor;
                            reconstruction.accumulation_frames = point.accumulation_frames;
                            reconstruction.reset = true;
                        }
                    }
                },
            },
            renzora::bool_field!("Restart History", CloudReconstruction, reset),
        ],
    }
}

renzora::add!(AtmosphericsEditorPlugin, Editor);
