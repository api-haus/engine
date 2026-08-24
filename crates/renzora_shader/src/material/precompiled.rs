//! Compiled material artifact — written to disk by the editor every time a
//! `.material` graph is saved. Three files live side-by-side:
//!
//! * `foo.material`      — graph JSON (editor only; never shipped). Carries a
//!   `wgsl_path` field pointing at the compiled shader.
//! * `foo.wgsl`          — pure WGSL fragment shader emitted by codegen.
//! * `foo.wgsl.meta`     — JSON sidecar with everything the resolver needs
//!   that the WGSL alone can't express (texture
//!   bindings, parameters, alpha mode, …).
//!
//! At runtime / play mode, the resolver reads `foo.wgsl` + `foo.wgsl.meta`,
//! skips graph parsing and codegen entirely, and feeds the cached WGSL into
//! the `ExtendedMaterial<StandardMaterial, SurfaceGraphExt>` asset.
//!
//! The `.wgsl.meta` carries a `source_material` back-reference so an asset
//! browser can find the graph that produced a given `.wgsl` when the user
//! moves or renames it.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::codegen::{self, MaterialParam, TextureBinding};
use super::graph::{AlphaMode, MaterialDomain, MaterialGraph};
use super::validate;
use renzora::InvalidShaderPolicy;

/// Metadata sidecar stored next to a compiled `.wgsl`. Captures the codegen
/// outputs that a runtime needs to assemble a `GraphMaterial` without
/// re-parsing the source graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledMaterialMeta {
    /// Project-relative path of the `.material` graph that produced this
    /// `.wgsl`. The editor's move tracker uses it to find the parent graph
    /// when a `.wgsl` is renamed or relocated.
    pub source_material: String,
    pub domain: MaterialDomain,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    pub requires_transmission: bool,
    pub texture_bindings: Vec<TextureBinding>,
    pub parameters: Vec<MaterialParam>,
}

/// Filesystem path of the meta sidecar for a given `.wgsl` path. Just
/// appends `.meta`, kept centralized so all callers stay in sync.
pub fn meta_path_for_wgsl(wgsl_path: &Path) -> PathBuf {
    let mut p = wgsl_path.as_os_str().to_owned();
    p.push(".meta");
    PathBuf::from(p)
}

/// Default `.wgsl` location for a `.material` at `material_fs_path`: same
/// directory, same stem, `.wgsl` extension. Editors that want to put the
/// compiled output somewhere else can compute their own path and assign it
/// to [`MaterialGraph::wgsl_path`] before calling [`save_compiled`].
pub fn default_wgsl_path_for_material(material_fs_path: &Path) -> PathBuf {
    material_fs_path.with_extension("wgsl")
}

/// Compute a project-relative version of `fs_path`, normalised to forward
/// slashes. Returns the absolute path stringified if it can't be made
/// relative — the caller normally avoids that case by ensuring the target
/// lives under `project_root`.
pub fn project_relative(project_root: &Path, fs_path: &Path) -> String {
    fs_path
        .strip_prefix(project_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| fs_path.to_string_lossy().replace('\\', "/"))
}

/// Run codegen on `graph`, write `.wgsl` + `.wgsl.meta` to disk, and update
/// `graph.wgsl_path` so a subsequent `serde_json::to_string_pretty(&graph)`
/// in the caller writes the link into the `.material` file.
///
/// Returns the codegen errors. An empty `Vec` means the artifacts were
/// written successfully. On codegen error or I/O failure no `.wgsl` is
/// written and `graph.wgsl_path` is cleared so the resolver doesn't follow
/// a stale link.
///
/// The caller is responsible for writing the updated graph back to
/// `material_fs_path` — this function only handles the compiled outputs.
pub fn save_compiled(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
) -> io::Result<Vec<String>> {
    save_compiled_with_policy(graph, project_root, material_fs_path, InvalidShaderPolicy::default())
}

/// [`save_compiled`] with an explicit policy for shaders that fail
/// validation.
///
/// Before anything is written, the compiled shaders go through
/// `validate::validate_compile_result` — naga, the same front end wgpu
/// compiles them with. A rejection means wgpu would have failed pipeline
/// creation at draw time with one log line; what happens next is the policy:
///
/// * [`InvalidShaderPolicy::Refuse`] — nothing is written, and
///   `graph.wgsl_path` is *kept* pointing at the last-good `.wgsl` (unlike a
///   codegen error, which clears it): the saved `.material` then degrades to
///   the previously compiled shader instead of the broken new one. The
///   validation errors are returned.
/// * [`InvalidShaderPolicy::WriteAnyway`] — the invalid `.wgsl` is written
///   (for inspecting codegen output), and the validation errors are still
///   returned. A non-empty `Vec` therefore no longer implies "not written"
///   under this policy.
pub fn save_compiled_with_policy(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
    policy: InvalidShaderPolicy,
) -> io::Result<Vec<String>> {
    let result = codegen::compile_with_functions(graph, None);
    if !result.errors.is_empty() {
        graph.wgsl_path = None;
        return Ok(result.errors);
    }

    match validate::validate_compile_result(&result) {
        Ok(()) => {}
        Err(errors) if policy == InvalidShaderPolicy::Refuse => {
            return Ok(errors.iter().map(|e| e.to_string()).collect());
        }
        Err(errors) => {
            // WriteAnyway: fall through to the writes, then report.
            let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            write_artifacts(graph, project_root, material_fs_path, result)?;
            return Ok(messages);
        }
    }

    write_artifacts(graph, project_root, material_fs_path, result)?;
    Ok(Vec::new())
}

/// The write half of [`save_compiled_with_policy`]: `.wgsl` + `.wgsl.meta`
/// to disk, `graph.wgsl_path` updated to the project-relative link.
fn write_artifacts(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
    result: codegen::CompileResult,
) -> io::Result<()> {
    let wgsl_fs_path = default_wgsl_path_for_material(material_fs_path);
    let meta_fs_path = meta_path_for_wgsl(&wgsl_fs_path);

    let meta = CompiledMaterialMeta {
        source_material: project_relative(project_root, material_fs_path),
        domain: result.domain,
        alpha_mode: graph.alpha_mode,
        double_sided: graph.double_sided,
        requires_transmission: result.requires_transmission,
        texture_bindings: result.texture_bindings,
        parameters: result.parameters,
    };

    if let Some(parent) = wgsl_fs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&wgsl_fs_path, result.fragment_shader.as_bytes())?;
    let meta_json = serde_json::to_string_pretty(&meta).map_err(io::Error::other)?;
    std::fs::write(&meta_fs_path, meta_json.as_bytes())?;

    graph.wgsl_path = Some(project_relative(project_root, &wgsl_fs_path));
    Ok(())
}

/// One-shot: run [`save_compiled`] then serialise the updated `graph` to a
/// pretty JSON string. Editor save sites use this to produce the `.material`
/// JSON they then write to disk.
pub fn save_compiled_and_serialize(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
) -> io::Result<(String, Vec<String>)> {
    save_compiled_and_serialize_with_policy(
        graph,
        project_root,
        material_fs_path,
        InvalidShaderPolicy::default(),
    )
}

/// [`save_compiled_and_serialize`] with an explicit invalid-shader policy.
/// The graph JSON is produced regardless — a refused `.wgsl` never costs the
/// user their graph edits.
pub fn save_compiled_and_serialize_with_policy(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
    policy: InvalidShaderPolicy,
) -> io::Result<(String, Vec<String>)> {
    let errors = save_compiled_with_policy(graph, project_root, material_fs_path, policy)?;
    let json = serde_json::to_string_pretty(graph).map_err(io::Error::other)?;
    Ok((json, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::graph::{MaterialDomain, PinValue};

    fn custom_code_graph(code: &str) -> MaterialGraph {
        let mut graph = MaterialGraph::new("t", MaterialDomain::Surface);
        let id = graph.add_node("custom/code", [0.0, 0.0]);
        graph
            .get_node_mut(id)
            .unwrap()
            .input_values
            .insert("code".to_string(), PinValue::String(code.to_string()));
        let output = graph.output_node().unwrap().id;
        graph.connect(id, "result", output, "base_color");
        graph
    }

    /// The Phase-3 contract: a graph whose shader naga rejects must leave the
    /// last-good `.wgsl` on disk, keep the graph's `wgsl_path` pointing at
    /// it, and say why — and the WriteAnyway escape hatch must still work.
    /// The broken shader comes from a `custom/code` snippet, the one place a
    /// user can still author invalid WGSL after the node-level fixes.
    #[test]
    fn invalid_graph_never_overwrites_last_good_wgsl() {
        let dir = std::env::temp_dir().join(format!(
            "renzora_precompiled_test_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let material_path = dir.join("t.material");
        let wgsl_path = dir.join("t.wgsl");

        // A good save establishes the last-good artifact.
        let mut graph = custom_code_graph("result = a;");
        let errors = save_compiled(&mut graph, &dir, &material_path).unwrap();
        assert!(errors.is_empty(), "errors: {errors:?}");
        let good_wgsl = std::fs::read_to_string(&wgsl_path).unwrap();
        let good_link = graph.wgsl_path.clone();

        // Break the snippet, save again: refused, reported, nothing touched.
        graph.get_node_mut(2).unwrap().input_values.insert(
            "code".to_string(),
            PinValue::String("result = vec4<f32>(;".to_string()),
        );
        let errors = save_compiled(&mut graph, &dir, &material_path).unwrap();
        assert!(!errors.is_empty(), "the broken snippet must be reported");
        assert_eq!(
            std::fs::read_to_string(&wgsl_path).unwrap(),
            good_wgsl,
            "a refused save must leave the previous .wgsl intact"
        );
        assert_eq!(
            graph.wgsl_path, good_link,
            "the graph must keep pointing at the last-good shader"
        );

        // WriteAnyway (codegen debugging): the broken artifact lands on disk
        // and the errors are still reported.
        let errors = save_compiled_with_policy(
            &mut graph,
            &dir,
            &material_path,
            InvalidShaderPolicy::WriteAnyway,
        )
        .unwrap();
        assert!(!errors.is_empty());
        assert_ne!(std::fs::read_to_string(&wgsl_path).unwrap(), good_wgsl);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
