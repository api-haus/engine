//! Runs generated material shaders through naga — the same compiler wgpu uses,
//! at the version wgpu links.
//!
//! wgpu already rejects bad WGSL, but only at draw time and only for a
//! material something is drawing. Most node types are in no shipped asset, so
//! their generated code reached no compiler at all. Running it here puts the
//! errors in front of CI and in the graph panel while you edit.
//!
//! Three callers — the tests below, `precompiled::save_compiled`, and the
//! editor panel — all print the strings this module returns, so they cannot
//! disagree.
//!
//! # What the stubs cost you
//!
//! Generated shaders open with Bevy `#import` lines that naga does not
//! understand. Those lines are stripped and [`FRAGMENT_STUB`] / [`VERTEX_STUB`]
//! stand in for what they would have brought. The stubs are hand-copied from
//! bevy_pbr 0.19.1, so if Bevy renames a field the real shader breaks and this
//! keeps passing. It checks our code, not the imports. Nothing here sees the
//! Rust-side bind group layout either — only the driver sees both sides of
//! that.

use naga::valid::{Capabilities, ValidationFlags, Validator};

use super::codegen::CompileResult;

#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Which generated shader failed: `"fragment"` or `"vertex"`.
    pub shader: &'static str,
    /// Which pipeline-define configuration was being validated, e.g.
    /// `"default"` or `"VERTEX_UVS_A,VERTEX_COLORS"`.
    pub defines: String,
    /// codespan-rendered diagnostic against the (stubbed) shader source.
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{} shader, defines: {}] {}",
            self.shader, self.defines, self.message
        )
    }
}

/// Validate both shaders of a [`CompileResult`] through naga.
///
/// The fragment shader is validated twice — once with no pipeline defines and
/// once with `VERTEX_UVS_A,VERTEX_COLORS` — because codegen's `#ifdef` blocks
/// pick different statements depending on which mesh attributes exist, and
/// both branches ship. The prepass defines (`DEPTH_PREPASS`, …) are *not*
/// exercised: those branches only reference stubbed prepass helpers, so
/// validating them would test the stub, not our code.
///
/// `Ok(())` means every configuration parsed and validated. `Err` carries one
/// entry per failing configuration, in source order.
pub fn validate_compile_result(result: &CompileResult) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // A codegen error already means the shader is known-incomplete; validating
    // its half-built output would report secondary noise instead of the cause.
    if !result.errors.is_empty() {
        errors.push(ValidationError {
            shader: "fragment",
            defines: String::new(),
            message: format!("codegen errors: {}", result.errors.join("; ")),
        });
        return Err(errors);
    }

    const FRAGMENT_CONFIGS: [&[&str]; 2] = [&[], &["VERTEX_UVS_A", "VERTEX_COLORS"]];
    for defines in FRAGMENT_CONFIGS {
        validate_shader(&result.fragment_shader, FRAGMENT_STUB, defines, "fragment", &mut errors);
    }

    if let Some(vertex) = &result.vertex_shader {
        validate_shader(vertex, VERTEX_STUB, &["VERTEX_UVS_A"], "vertex", &mut errors);
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn validate_shader(
    source: &str,
    stub: &str,
    defines: &[&str],
    stage: &'static str,
    errors: &mut Vec<ValidationError>,
) {
    let preprocessed = preprocess(source, stub, defines);
    let label = || defines.join(",");
    match naga::front::wgsl::parse_str(&preprocessed) {
        Ok(module) => {
            let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
            if let Err(err) = validator.validate(&module) {
                errors.push(ValidationError {
                    shader: stage,
                    defines: label(),
                    // Debug dump, because validation errors have no `emit_to_string` like parse errors do.
                    message: format!("{err:?}"),
                });
            }
        }
        Err(err) => errors.push(ValidationError {
            shader: stage,
            defines: label(),
            message: err.emit_to_string(&preprocessed),
        }),
    }
}

/// Turn generated WGSL-with-Bevy-directives into something naga can parse:
/// resolve `#ifdef`/`#else`/`#endif` against `defines`, drop `#import` lines,
/// rewrite `pbr_functions::foo`-style paths (illegal in WGSL identifiers —
/// Bevy's own preprocessor rewrites them the same way, to prefixed names),
/// and prepend the stub of the imported declarations.
fn preprocess(source: &str, stub: &str, defines: &[&str]) -> String {
    let mut out = String::with_capacity(source.len() + stub.len());
    // #ifdef nesting depth is 1 in every shader codegen emits; a Vec is used
    // anyway so a future nested block degrades gracefully instead of
    // corrupting the output.
    let mut stack: Vec<bool> = Vec::new();
    let mut active = true;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(name) = trimmed.strip_prefix("#ifdef") {
            let name = name.trim();
            stack.push(active);
            active = active && defines.contains(&name);
            continue;
        }
        if trimmed.starts_with("#else") {
            let outer = stack.last().copied().unwrap_or(true);
            active = outer && !active;
            continue;
        }
        if trimmed.starts_with("#endif") {
            active = stack.pop().unwrap_or(true);
            continue;
        }
        if !active || trimmed.starts_with("#import") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    // Bevy's naga_oil-based preprocessor rewrites `module::item` to
    // `module_item` when resolving imports; mirror that for the paths codegen
    // can emit. Order matters: longest prefix first.
    let rewritten = out
        .replace("bevy_pbr::prepass_utils::", "stub_")
        .replace("pbr_functions::", "stub_")
        .replace("mesh_functions::", "stub_");

    format!("{stub}\n{rewritten}")
}

/// Stub of everything `#import` would have brought into the fragment shader.
/// Field names and signatures mirror bevy_pbr 0.19.1
/// (`forward_io.wgsl`, `pbr_types.wgsl`, `pbr_fragment.wgsl`,
/// `pbr_functions.wgsl`, `prepass/prepass_utils.wgsl`) and bevy_render 0.19.1
/// (`view/view.wgsl`, `globals.wgsl`) — see the module doc for what that
/// costs.
///
/// Only the members generated code can actually reference are declared: the
/// full `StandardMaterial` has 25 fields, the 14 here are the ones codegen
/// writes to.
const FRAGMENT_STUB: &str = r#"
struct View {
    clip_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    world_position: vec3<f32>,
    viewport: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: View;

struct Globals {
    time: f32,
    delta_time: f32,
    frame_count: u32,
};
@group(0) @binding(1) var<uniform> globals: Globals;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    // MeshPipeline defines VERTEX_OUTPUT_INSTANCE_INDEX unconditionally
    // (mesh.rs), so the real struct always carries this for our materials.
    @location(6) @interpolate(flat) instance_index: u32,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

struct StandardMaterial {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
    reflectance: vec3<f32>,
    perceptual_roughness: f32,
    metallic: f32,
    diffuse_transmission: f32,
    specular_transmission: f32,
    thickness: f32,
    ior: f32,
    attenuation_distance: f32,
    clearcoat: f32,
    clearcoat_perceptual_roughness: f32,
    anisotropy_strength: f32,
    anisotropy_rotation: vec2<f32>,
};

struct PbrInput {
    material: StandardMaterial,
    diffuse_occlusion: vec3<f32>,
    world_position: vec4<f32>,
    world_normal: vec3<f32>,
    N: vec3<f32>,
    V: vec3<f32>,
};

fn pbr_input_from_standard_material(in: VertexOutput, is_front: bool) -> PbrInput {
    var pbr_input: PbrInput;
    return pbr_input;
}

fn stub_alpha_discard(material: StandardMaterial, output_color: vec4<f32>) -> vec4<f32> {
    return output_color;
}

fn stub_apply_pbr_lighting(in: PbrInput) -> vec4<f32> {
    return in.material.base_color;
}

fn stub_main_pass_post_lighting_processing(pbr_input: PbrInput, input_color: vec4<f32>) -> vec4<f32> {
    return input_color;
}

// `saturate` reaches the real shader transitively through the pbr_functions
// import; codegen calls it bare.
fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

// Referenced only under the prepass #ifdef branches, which validation never
// defines — declared so the day those branches are exercised the stubs exist.
fn stub_prepass_depth(frag_coord: vec4<f32>, sample_index: u32) -> f32 {
    return 1.0;
}
fn stub_prepass_normal(frag_coord: vec4<f32>, sample_index: u32) -> vec3<f32> {
    return vec3<f32>(0.0, 0.0, 1.0);
}
fn stub_prepass_motion_vector(frag_coord: vec4<f32>, sample_index: u32) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}

// Imported by transmission graphs.
@group(0) @binding(2) var view_transmission_texture: texture_2d<f32>;
@group(0) @binding(3) var view_transmission_sampler: sampler;
// Imported by environment-map graphs (single-map branch of
// MULTIPLE_LIGHT_PROBES_IN_ARRAY, the one validation takes).
@group(0) @binding(4) var specular_environment_map: texture_cube<f32>;
@group(0) @binding(5) var environment_map_sampler: sampler;

// `mesh_functions::get_world_from_local` — referenced by
// `input/object_position` under the gated `mesh_functions` import.
fn stub_get_world_from_local(instance_index: u32) -> mat4x4<f32> {
    return mat4x4<f32>();
}
"#;

/// Stub for the Vegetation domain's vertex stage (`mesh_functions` +
/// `forward_io::Vertex` + `mesh_view_bindings::{view, globals}`).
const VERTEX_STUB: &str = r#"
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct View {
    clip_from_world: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> view: View;

struct Globals {
    time: f32,
    delta_time: f32,
    frame_count: u32,
};
@group(0) @binding(1) var<uniform> globals: Globals;

fn stub_get_world_from_local(instance_index: u32) -> mat4x4<f32> {
    return mat4x4<f32>();
}
fn stub_mesh_position_local_to_world(world_from_local: mat4x4<f32>, vertex_position: vec4<f32>) -> vec4<f32> {
    return world_from_local * vertex_position;
}
fn stub_mesh_normal_local_to_world(vertex_normal: vec3<f32>, instance_index: u32) -> vec3<f32> {
    return vertex_normal;
}
"#;

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::codegen;
    use crate::material::graph::{MaterialGraph, PinDir, PinType};
    use crate::material::nodes;
    use std::path::PathBuf;

    fn materials_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/materials")
    }

    #[test]
    fn every_shipped_material_compiles_to_valid_wgsl() {
        let dir = materials_dir();
        let mut count = 0;
        let mut failures = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("material") {
                continue;
            }
            count += 1;
            let json = std::fs::read_to_string(&path).unwrap();
            let graph: MaterialGraph = match serde_json::from_str(&json) {
                Ok(g) => g,
                Err(e) => {
                    failures.push(format!("{}: graph JSON does not parse: {e}", path.display()));
                    continue;
                }
            };
            let result = codegen::compile(&graph);
            if let Err(errors) = validate_compile_result(&result) {
                for err in errors {
                    failures.push(format!("{}: {err}", path.display()));
                }
            }
        }
        assert!(count > 0, "no .material files found under {}", dir.display());
        assert!(
            failures.is_empty(),
            "{count} materials, {} validation failures:\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    /// Build a single-node graph exercising `node_type`: add the node, wire
    /// every one of its output pins into a compatible input on the output
    /// node, and compile. Nodes whose outputs no output pin accepts
    /// (Texture2D, Sampler, String) are wired through one intermediate node
    /// that does accept them — codegen only visits nodes reachable from the
    /// output node, so an unconnected node would generate nothing and the
    /// test would pass vacuously.
    fn graph_exercising(node_type: &str) -> Option<MaterialGraph> {
        let def = nodes::node_def(node_type)?;
        let mut graph = MaterialGraph::new("coverage", crate::material::graph::MaterialDomain::Surface);
        let node_id = graph.add_node(node_type, [0.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;

        let outputs: Vec<_> = (def.pins)()
            .into_iter()
            .filter(|p| p.direction == PinDir::Output)
            .collect();

        for pin in &outputs {
            if try_wire(&mut graph, node_id, &pin.name, pin.pin_type, output_id).is_none() {
                // No route to the output node at all — leave the pin
                // disconnected; other pins may still reach it.
                continue;
            }
        }
        Some(graph)
    }

    /// Wire `from_pin` on `from_node` toward the output node, directly or via
    /// one intermediate. Returns the connection made.
    fn try_wire(
        graph: &mut MaterialGraph,
        from_node: u64,
        from_pin: &str,
        from_type: PinType,
        output_id: u64,
    ) -> Option<()> {
        // Direct: a compatible input on the output node.
        let output_def = nodes::node_def(&graph.get_node(output_id)?.node_type)?;
        if let Some(target) = (output_def.pins)().into_iter().find(|p| {
            p.direction == PinDir::Input && PinType::compatible(from_type, p.pin_type)
        }) {
            graph.connect(from_node, from_pin, output_id, &target.name);
            return Some(());
        }
        // Via one intermediate node that accepts this type and can itself
        // reach the output node.
        for mid in nodes::ALL_NODES {
            if mid.node_type.starts_with("output/") || mid.node_type.starts_with("function/") {
                continue;
            }
            let mid_pins = (mid.pins)();
            let Some(mid_input) = mid_pins.iter().find(|p| {
                p.direction == PinDir::Input && PinType::compatible(from_type, p.pin_type)
            }) else {
                continue;
            };
            let Some(mid_output) = mid_pins.iter().find(|p| {
                p.direction == PinDir::Output
                    && (output_def.pins)().iter().any(|t| {
                        t.direction == PinDir::Input && PinType::compatible(p.pin_type, t.pin_type)
                    })
            }) else {
                continue;
            };
            let mid_id = graph.add_node(mid.node_type, [100.0, 0.0]);
            let mid_input_name = mid_input.name.clone();
            let mid_output_name = mid_output.name.clone();
            graph.connect(from_node, from_pin, mid_id, &mid_input_name);
            let target = (output_def.pins)()
                .into_iter()
                .find(|t| {
                    t.direction == PinDir::Input
                        && PinType::compatible(mid_output.pin_type, t.pin_type)
                })?;
            graph.connect(mid_id, &mid_output_name, output_id, &target.name);
            return Some(());
        }
        None
    }

    #[test]
    fn every_node_type_generates_valid_wgsl() {
        let mut failures = Vec::new();
        let mut count = 0;
        for def in nodes::ALL_NODES {
            let node_type = def.node_type;
            // Output nodes are the sink the harness builds around, and
            // function bracket nodes only codegen inside a MaterialFunction.
            if node_type.starts_with("output/") || node_type.starts_with("function/") {
                continue;
            }
            count += 1;
            let Some(graph) = graph_exercising(node_type) else {
                failures.push(format!("{node_type}: unknown to nodes::node_def"));
                continue;
            };
            let result = codegen::compile(&graph);
            if let Err(errors) = validate_compile_result(&result) {
                for err in errors {
                    failures.push(format!("{node_type}: {err}"));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{count} node types exercised, {} validation failures:\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    /// The §2.2 defect: `param/bool` is the only Bool *output* pin in the
    /// node set, its codegen emits a real WGSL `bool`, and `cast_expr` used
    /// to pass it unchanged into a float pin. Wiring it into
    /// `output/surface.metallic` must produce a shader naga accepts.
    #[test]
    fn param_bool_wired_into_float_pin_validates() {
        let mut graph = MaterialGraph::new(
            "bool_into_float",
            crate::material::graph::MaterialDomain::Surface,
        );
        let param = graph.add_node("param/bool", [0.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(param, "value", output_id, "metallic");

        let result = codegen::compile(&graph);
        validate_compile_result(&result).unwrap_or_else(|errors| {
            panic!(
                "param/bool → metallic must validate:\n{}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
}
