//! GPU e2e: a one-frame dropout is invisible to any CPU-side assertion — the
//! resolver compiles once — so this compares framebuffers instead, with the
//! `.wgsl` watcher running, because that is what made the material blink.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages,
};
use renzora_shader::material::graph::{MaterialDomain, MaterialGraph, PinValue};
use renzora_shader::material::material_ref::MaterialRef;

const SIZE: u32 = 64;
const FRAMES: usize = 300;

/// Every frame the readback delivered, in arrival order.
#[derive(Resource, Default)]
struct Frames(Vec<Vec<u8>>);

/// A maximal stretch of consecutive frames with identical pixels.
struct Run {
    start: usize,
    end: usize,
    label: usize,
}

/// Dropping an `App` that holds a live `RenderDevice` segfaults inside the
/// Vulkan driver on teardown; the process is exiting anyway.
fn leak(app: App) {
    std::mem::forget(app);
}

fn target_image(images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT;
    images.add(image)
}

/// A graph the clock cannot move: one static Custom Code snippet on `base_color`.
fn static_graph() -> MaterialGraph {
    let mut graph = MaterialGraph::new("NewMaterial", MaterialDomain::Surface);
    let node = graph.add_node("custom/code", [0.0, 0.0]);
    graph
        .get_node_mut(node)
        .unwrap()
        .input_values
        .insert(
            "code".to_string(),
            PinValue::String("result = vec4<f32>(0.2, 0.4, 0.8, 1.0);".to_string()),
        );
    let output = graph.output_node().unwrap().id;
    graph.connect(node, "result", output, "base_color");
    graph
}

#[test]
fn a_graph_material_renders_identically_every_frame() {
    let Some(mut app) = renzora_test_harness::gpu_app_with(|app| {
        app.add_plugins((
            renzora_shader::ShaderPlugin,
            renzora_shader::material::runtime::GraphMaterialPlugin,
            renzora_shader::material::resolver::MaterialResolverPlugin,
            renzora_shader_editor::hot_reload::WgslHotReloadPlugin,
        ));
    }) else {
        return;
    };

    let dir = std::env::temp_dir().join(format!("renzora_flicker_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Compiled on disk the way an editor save compiles it: `.material`, plus
    // the `.wgsl` and `.meta` the watcher and the resolver read.
    let material_path = dir.join("NewMaterial.material");
    let mut graph = static_graph();
    let (json, _) = renzora_shader::material::precompiled::save_compiled_and_serialize(
        &mut graph,
        &dir,
        &material_path,
    )
    .unwrap();
    std::fs::write(&material_path, json).unwrap();

    app.insert_resource(renzora::CurrentProject {
        path: dir.clone(),
        config: Default::default(),
    });
    app.init_resource::<Frames>();

    let target = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        target_image(&mut images)
    };
    let mesh = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Sphere::new(1.0).mesh().uv(32, 18))
    };

    app.world_mut().spawn((
        Mesh3d(mesh),
        MaterialRef("NewMaterial.material".to_string()),
        Transform::default(),
    ));
    app.world_mut().spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    app.world_mut().spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        // The editor viewport's passes, not a bare forward camera: the shadow
        // and prepass pipelines are separately specialized, and a graph
        // material has to hold up in all of them.
        (
            bevy::core_pipeline::prepass::DepthPrepass,
            bevy::core_pipeline::prepass::NormalPrepass,
            bevy::core_pipeline::prepass::MotionVectorPrepass,
        ),
        bevy::camera::RenderTarget::Image(target.clone().into()),
        Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    app.world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(|trigger: On<ReadbackComplete>, mut frames: ResMut<Frames>| {
            frames.0.push(trigger.event().data.clone());
        });

    // Wall clock, not frame count: the watcher's debounce is 200ms, so a run
    // that finishes in a second never sees the second reload.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut drawn = 0;
    while std::time::Instant::now() < deadline || drawn < FRAMES {
        // Something in the editor keeps this file open — a code tab, a
        // thumbnail, the resolver itself. Reading it must not disturb what is
        // on screen.
        let _ = std::fs::read(dir.join("NewMaterial.wgsl"));
        app.update();
        drawn += 1;
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    let frames = std::mem::take(&mut app.world_mut().resource_mut::<Frames>().0);
    assert!(
        frames.len() > FRAMES / 2,
        "only {} of {FRAMES} frames read back",
        frames.len()
    );

    // A watcher that never installed makes this gate vacuous.
    assert!(
        app.world()
            .get_resource::<renzora_shader_editor::hot_reload::WgslHotReload>()
            .is_some_and(|w| w.project_root == dir),
        "the `.wgsl` watcher must be running, or nothing here is under test"
    );
    let compiles = app
        .world()
        .resource::<renzora_shader::material::perf::MaterialPerfStats>()
        .total_compiles;
    let _ = std::fs::remove_dir_all(&dir);
    leak(app);
    println!("{compiles} compiles across {} frames", frames.len());
    assert_eq!(
        compiles, 1,
        "nobody edited the material, so it must compile exactly once"
    );

    // Group identical frames, then read the timeline as runs. A material that
    // renders and stays rendered ends in one long final run; one that blinks
    // alternates between two classes forever.
    let mut classes: Vec<&Vec<u8>> = Vec::new();
    let labels: Vec<usize> = frames
        .iter()
        .map(|f| match classes.iter().position(|c| *c == f) {
            Some(i) => i,
            None => {
                classes.push(f);
                classes.len() - 1
            }
        })
        .collect();

    let mut runs: Vec<Run> = Vec::new();
    for (i, &label) in labels.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if run.label == label => run.end = i,
            _ => runs.push(Run { start: i, end: i, label }),
        }
    }

    let out = std::env::temp_dir().join(format!("renzora_flicker_frames_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&out);
    for (i, class) in classes.iter().enumerate() {
        save_png(&out.join(format!("class{i}.png")), class);
    }
    println!("{} frames, {} distinct classes", frames.len(), classes.len());
    println!("runs (first..last = class):");
    for run in &runs {
        println!("  {:>3}..{:<3} = {}", run.start, run.end, run.label);
    }

    // The last run is the settled state. Everything before it is startup.
    let settled = runs.last().expect("frames were read back").start;
    let blinks = runs
        .iter()
        .filter(|run| run.start > runs[0].end && run.start < settled)
        .count();
    assert!(
        blinks == 0 && runs.len() <= 2,
        "the render changes {} times across {} frames — see {}",
        runs.len() - 1,
        frames.len(),
        out.display()
    );
}

fn save_png(path: &std::path::Path, data: &[u8]) {
    let len = (SIZE * SIZE) as usize * 4;
    let image = image::RgbaImage::from_raw(SIZE, SIZE, data[..len].to_vec())
        .expect("readback is one rgba8 pixel per texel");
    image.save(path).expect("write png");
}
