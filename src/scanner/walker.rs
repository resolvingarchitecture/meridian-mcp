use ignore::WalkBuilder;
use std::path::PathBuf;

pub const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    "coverage",
    "target",
    ".idea",
    ".vscode",
    "out",
    "bin",
    "obj",
];

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
