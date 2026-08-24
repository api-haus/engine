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

/// What a save-compile has to say. `errors` non-empty means no `.wgsl` was
/// written. `warnings` means one was, but the graph is on borrowed time.
#[derive(Debug, Default)]
pub struct SaveReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Run codegen on `graph`, write `.wgsl` + `.wgsl.meta` to disk, and update
/// `graph.wgsl_path` so a subsequent `serde_json::to_string_pretty(&graph)`
/// in the caller writes the link into the `.material` file.
///
/// On codegen error or I/O failure no `.wgsl` is written and `graph.wgsl_path`
/// is cleared so the resolver doesn't follow a stale link.
///
/// The caller is responsible for writing the updated graph back to
/// `material_fs_path` — this function only handles the compiled outputs.
pub fn save_compiled(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
) -> io::Result<SaveReport> {
    let result = codegen::compile_with_functions(graph, None);
    if !result.errors.is_empty() {
        graph.wgsl_path = None;
        return Ok(SaveReport {
            errors: result.errors,
            warnings: result.warnings,
        });
    }

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
    Ok(SaveReport {
        errors: Vec::new(),
        warnings: result.warnings,
    })
}

/// One-shot: run [`save_compiled`] then serialise the updated `graph` to a
/// pretty JSON string. Editor save sites use this to produce the `.material`
/// JSON they then write to disk.
pub fn save_compiled_and_serialize(
    graph: &mut MaterialGraph,
    project_root: &Path,
    material_fs_path: &Path,
) -> io::Result<(String, SaveReport)> {
    let report = save_compiled(graph, project_root, material_fs_path)?;
    let json = serde_json::to_string_pretty(graph).map_err(io::Error::other)?;
    Ok((json, report))
}
