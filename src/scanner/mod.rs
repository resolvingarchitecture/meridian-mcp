use crate::models::{
    ArchitectureComponent, ArchitectureComponentType, ArchitectureObservations, DocumentInput,
    Domain,
};
use anyhow::Result;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub mod adrs;
pub mod documents;
pub mod walker;

pub use crate::models::ArchitectureModel;

// ── Main scan entry points ────────────────────────────────────────────────────

pub fn scan(root: &Path) -> Result<ArchitectureComponent> {
    let paths = walker::collect_paths(root);
    let harvested_documents = documents::harvest(&paths);
    let model = documents::discover_architecture_model(Uuid::new_v4(), &harvested_documents);
    let observations = project_observations_for_scan(&paths, model.global_observations);

    Ok(ArchitectureComponent {
        component_id: "application-root".to_string(),
        name: root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Application")
            .to_string(),
        component_type: ArchitectureComponentType::Application,
        domain: Domain::Application,
        root: Some(root.to_string_lossy().to_string()),
        language: None,
        framework: None,
        observations,
        evidence_refs: model
            .evidence
            .iter()
            .map(|evidence| evidence.evidence_id.clone())
            .collect(),
        confidence: 0.60,
    })
}

pub fn scan_documents(root: &Path) -> Result<Vec<DocumentInput>> {
    let paths = walker::collect_paths(root);
    Ok(documents::harvest(&paths))
}

pub fn document_input_for_path(path: &Path, content: &str) -> Option<DocumentInput> {
    documents::document_input_for_path(path, content)
}

fn project_observations_for_scan(
    paths: &[PathBuf],
    mut observations: ArchitectureObservations,
) -> ArchitectureObservations {
    let source_layers = source_folder_layers(paths);

    for layer in source_layers {
        if !observations.layers.contains(&layer) {
            observations.layers.push(layer);
        }
    }

    observations.layer_order = ordered_layers_for_tests(&observations.layers);

    if observations.style.is_none() {
        observations.style = Some("modular".to_string());
    }

    if observations.layers.iter().any(|layer| layer == "domain")
        && observations.layers.iter().any(|layer| layer == "repositories")
    {
        observations.style = Some("layered_ddd".to_string());
    }

    observations
}

fn source_folder_layers(paths: &[PathBuf]) -> Vec<String> {
    let mut layers = BTreeSet::new();

    for path in paths {
        let normalized = path.to_string_lossy().replace('\\', "/");

        for layer in [
            "controllers",
            "services",
            "domain",
            "repositories",
            "infra",
            "infrastructure",
            "adapters",
            "ports",
            "application",
        ] {
            if normalized.contains(&format!("/{layer}/")) {
                layers.insert(layer.to_string());
            }
        }
    }

    layers.into_iter().collect()
}

fn ordered_layers_for_tests(layers: &[String]) -> Vec<String> {
    let preferred_order = [
        "controllers",
        "api",
        "services",
        "service",
        "application",
        "repositories",
        "persistence",
        "infra",
        "infrastructure",
        "adapters",
        "ports",
        "domain",
    ];

    let mut ordered = Vec::new();

    for preferred in preferred_order {
        if layers.iter().any(|layer| layer == preferred) {
            ordered.push(preferred.to_string());
        }
    }

    for layer in layers {
        if !ordered.contains(layer) {
            ordered.push(layer.clone());
        }
    }

    ordered
}