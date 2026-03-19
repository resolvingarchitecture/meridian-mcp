use crate::scanner::{IGNORED_DIRS, KNOWN_LAYERS};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::PathBuf;

/// Collect all paths in the project, respecting .gitignore.
/// Returns paths only — no file content read at this stage.
pub fn collect_paths(root: &std::path::Path) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !IGNORED_DIRS.contains(&name)
        })
        .build()
        .filter_map(|entry| entry.ok().map(|e| e.into_path()))
        .collect()
}

/// Infer architectural layers from directory names in the project.
/// Only matches known layer names — not every directory is a layer.
pub fn infer_layers(paths: &[PathBuf]) -> Vec<String> {
    let mut seen = HashSet::new();
    for path in paths {
        for component in path.components() {
            let name = component.as_os_str().to_str().unwrap_or("");
            if KNOWN_LAYERS.contains(&name) {
                seen.insert(name.to_string());
            }
        }
    }
    // Return in a stable, meaningful order
    let mut layers: Vec<String> = seen.into_iter().collect();
    layers.sort_by_key(|l| {
        KNOWN_LAYERS.iter().position(|&k| k == l).unwrap_or(99)
    });
    layers
}

/// Get the layer name a file belongs to, based on its path components.
pub fn get_layer(path: &std::path::Path, layers: &[String]) -> Option<String> {
    for component in path.components() {
        let name = component.as_os_str().to_str().unwrap_or("");
        if layers.iter().any(|l| l == name) {
            return Some(name.to_string());
        }
    }
    None
}

/// Sample up to `limit` source files for pattern detection.
/// Prefers files in known layer directories.
pub fn sample_source_files(paths: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    let source_extensions = ["ts", "tsx", "js", "jsx", "java", "py", "go", "rs"];
    paths.iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| source_extensions.contains(&e))
                .unwrap_or(false)
        })
        .take(limit)
        .cloned()
        .collect()
}
