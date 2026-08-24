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
/// Validates through naga and writes either way. Holding a broken shader back
/// saves nothing: a `.material` with no usable `.wgsl` falls through to
/// `resolver`'s live codegen, which builds the same broken shader in memory.
/// Errors returned here mean "this will not compile", not "nothing written".
pub fn save_compiled(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
) -> io::Result<Vec<String>> {
    let result = codegen::compile_with_functions(graph, None);
    if !result.errors.is_empty() {
        graph.wgsl_path = None;
        return Ok(result.errors);
    }

    let errors = match validate::validate_compile_result(&result) {
        Ok(()) => Vec::new(),
        Err(errors) => errors.iter().map(|e| e.to_string()).collect(),
    };
    write_artifacts(graph, project_root, material_fs_path, result)?;
    Ok(errors)
}

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
    let errors = save_compiled(graph, project_root, material_fs_path)?;
    let json = serde_json::to_string_pretty(graph).map_err(io::Error::other)?;
    Ok((json, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::graph::{MaterialDomain, PinValue};

    /// Returns the graph and the `custom/code` node's id, so a test can
    /// rewrite the snippet without hardcoding the id the graph handed out.
    fn custom_code_graph(code: &str) -> (MaterialGraph, u64) {
        let mut graph = MaterialGraph::new("t", MaterialDomain::Surface);
        let id = graph.add_node("custom/code", [0.0, 0.0]);
        graph
            .get_node_mut(id)
            .unwrap()
            .input_values
            .insert("code".to_string(), PinValue::String(code.to_string()));
        let output = graph.output_node().unwrap().id;
        graph.connect(id, "result", output, "base_color");
        (graph, id)
    }

    /// A graph whose shader naga rejects is written *and* reported. The
    /// broken WGSL comes from a `custom/code` snippet, the one place a user
    /// can still author invalid WGSL after the node-level fixes.
    #[test]
    fn invalid_graph_is_written_and_reported() {
        let dir = std::env::temp_dir().join(format!(
            "renzora_precompiled_test_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let material_path = dir.join("t.material");
        let wgsl_path = dir.join("t.wgsl");

        let (mut graph, code_node) = custom_code_graph("result = a;");
        let errors = save_compiled(&mut graph, &dir, &material_path).unwrap();
        assert!(errors.is_empty(), "errors: {errors:?}");
        let good_wgsl = std::fs::read_to_string(&wgsl_path).unwrap();

        graph.get_node_mut(code_node).unwrap().input_values.insert(
            "code".to_string(),
            PinValue::String("result = vec4<f32>(;".to_string()),
        );
        let errors = save_compiled(&mut graph, &dir, &material_path).unwrap();
        assert!(!errors.is_empty(), "the broken snippet must be reported");
        assert_ne!(
            std::fs::read_to_string(&wgsl_path).unwrap(),
            good_wgsl,
            "the broken shader must reach disk so it is inspectable and fails loudly"
        );
        assert!(
            graph.wgsl_path.is_some(),
            "a validation failure must not clear the link the way a codegen error does"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
