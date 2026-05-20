use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod adrs;
pub mod imports;
pub mod patterns;
pub mod walker;

// ── ArchModel — shared with Java backend via JSON contract ───────────────────
pub type ArchModel = ArchitectureModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureModel {
    pub model_id: String,
    pub context_id: Option<String>,
    pub root: Option<String>,
    pub scanned_at: u64,
    pub components: Vec<ArchitectureComponent>,
    pub relationships: Vec<ArchitectureRelationship>,
    pub global_observations: ArchitectureObservations,
    pub evidence: Vec<ArchitectureEvidence>,
    pub warnings: Vec<String>,
}

impl ArchitectureModel {
    pub fn new(root: Option<String>) -> Self {
        let scanned_at = current_epoch_secs();

        Self {
            model_id: format!("model-{scanned_at}"),
            context_id: None,
            root,
            scanned_at,
            components: Vec::new(),
            relationships: Vec::new(),
            global_observations: ArchitectureObservations::default(),
            evidence: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn from_component(component: ArchitectureComponent) -> Self {
        let root = component.root.clone();
        let mut model = Self::new(root);
        model.add_component(component);
        model
    }

    pub fn add_component(&mut self, component: ArchitectureComponent) {
        self.scanned_at = self.scanned_at.max(component.scanned_at);
        self.upsert_component(component);
        self.rebuild_global_observations();
    }

    pub fn component_ids(&self) -> impl Iterator<Item = &str> {
        self.components
            .iter()
            .map(|component| component.component_id.as_str())
    }

    pub fn has_adrs(&self) -> bool {
        self.global_observations.has_adrs()
    }

    pub fn style_summary(&self) -> &str {
        self.global_observations
            .style
            .as_deref()
            .unwrap_or("unknown")
    }

    pub fn layer_order_summary(&self) -> &[String] {
        &self.global_observations.layer_order
    }

    fn upsert_component(&mut self, component: ArchitectureComponent) {
        if let Some(existing) = self
            .components
            .iter_mut()
            .find(|existing| existing.component_id == component.component_id)
        {
            *existing = component;
        } else {
            self.components.push(component);
        }
    }

    fn rebuild_global_observations(&mut self) {
        let mut observations = ArchitectureObservations::default();

        for component in &self.components {
            extend_unique(&mut observations.layers, &component.observations.layers);
            extend_unique(
                &mut observations.layer_order,
                &component.observations.layer_order,
            );
            extend_unique(&mut observations.patterns, &component.observations.patterns);
            extend_unique(&mut observations.adrs, &component.observations.adrs);
            extend_unique(&mut observations.modules, &component.observations.modules);
            extend_unique(
                &mut observations.technologies,
                &component.observations.technologies,
            );
            extend_unique(&mut observations.risks, &component.observations.risks);
            extend_unique(
                &mut observations.conventions,
                &component.observations.conventions,
            );
        }

        observations.style = infer_global_style(&self.components);
        self.global_observations = observations;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureComponent {
    pub component_id: String,
    pub name: String,
    pub scanned_at: u64,
    pub component_type: ArchitectureComponentType,
    pub domain: ArchitectureDomain,
    pub root: Option<String>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub observations: ArchitectureObservations,
    pub evidence_refs: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchitectureComponentType {
    Application,
    Service,
    Library,
    Module,
    DataStore,
    Integration,
    Infrastructure,
    SecurityScope,
    EnterpriseScope,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchitectureDomain {
    Application,
    Integration,
    Data,
    Infrastructure,
    Security,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArchitectureObservations {
    pub layers: Vec<String>,
    pub layer_order: Vec<String>,
    pub style: Option<String>,
    pub patterns: Vec<String>,
    pub adrs: Vec<String>,
    pub modules: Vec<String>,
    pub technologies: Vec<String>,
    pub risks: Vec<String>,
    pub conventions: Vec<String>,
}

impl ArchitectureObservations {
    pub fn has_adrs(&self) -> bool {
        !self.adrs.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureRelationship {
    pub source_component_id: String,
    pub target_component_id: String,
    pub relationship_type: ArchitectureRelationshipType,
    pub protocol: Option<String>,
    pub evidence_refs: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArchitectureRelationshipType {
    Calls,
    PublishesTo,
    SubscribesTo,
    ReadsFrom,
    WritesTo,
    DependsOn,
    DeploysTo,
    AuthenticatesWith,
    Owns,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureEvidence {
    pub evidence_id: String,
    pub source_type: String,
    pub path: Option<String>,
    pub description: String,
    pub scanned_at: Option<u64>,
}

// ── Known layer names — used to identify architectural layers from dir names ──

pub const KNOWN_LAYERS: &[&str] = &[
    "controllers",
    "controller",
    "services",
    "service",
    "domain",
    "application",
    "repositories",
    "repository",
    "infra",
    "infrastructure",
    "adapters",
    "ports",
    "handlers",
    "usecases",
    "usecase",
];

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

// ── Main scan entry point ─────────────────────────────────────────────────────

pub fn scan(root: &Path) -> Result<ArchitectureComponent> {
    let all_paths = walker::collect_paths(root);
    let layers = walker::infer_layers(&all_paths);
    let edges = imports::parse_import_graph(root, &layers)?;
    let layer_order = imports::topo_sort(&edges, &layers);
    let patterns = patterns::detect(root, &all_paths);
    let adrs = adrs::harvest(root);
    let style = infer_style(&layers, &patterns);
    let scanned_at = current_epoch_secs();

    Ok(ArchitectureComponent {
        component_id: component_id_for_root(root),
        name: component_name_for_root(root),
        component_type: ArchitectureComponentType::Application,
        domain: ArchitectureDomain::Application,
        root: Some(root.to_string_lossy().to_string()),
        language: None,
        framework: None,
        scanned_at,
        observations: ArchitectureObservations {
            layers,
            layer_order,
            style: Some(style),
            patterns,
            adrs,
            modules: Vec::new(),
            technologies: Vec::new(),
            risks: Vec::new(),
            conventions: Vec::new(),
        },
        evidence_refs: Vec::new(),
        confidence: 0.7,
    })
}

pub fn infer_style(layers: &[String], patterns: &[String]) -> String {
    let has_domain = layers.iter().any(|l| l == "domain");
    let has_ports = layers.iter().any(|l| l == "ports" || l == "adapters");
    let has_app = layers.iter().any(|l| l == "application");
    let has_repo = patterns.iter().any(|p| p == "repository_pattern");

    if has_domain && has_ports {
        "hexagonal".to_string()
    } else if has_domain && has_app {
        "clean_architecture".to_string()
    } else if has_domain && has_repo {
        "layered_ddd".to_string()
    } else if has_domain {
        "layered".to_string()
    } else {
        "modular".to_string()
    }
}

fn current_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn component_name_for_root(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("architecture-component")
        .to_string()
}

fn component_id_for_root(root: &Path) -> String {
    let normalized = root.to_string_lossy();
    let slug: String = normalized
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = slug.trim_matches('-');

    if trimmed.is_empty() {
        "component-root".to_string()
    } else {
        format!("component-{trimmed}")
    }
}

fn infer_global_style(components: &[ArchitectureComponent]) -> Option<String> {
    let mut styles = components
        .iter()
        .filter_map(|component| component.observations.style.as_deref());

    let first = styles.next()?;

    if styles.all(|style| style == first) {
        Some(first.to_string())
    } else {
        Some("mixed".to_string())
    }
}

fn extend_unique(target: &mut Vec<String>, source: &[String]) {
    for value in source {
        if !target.contains(value) {
            target.push(value.clone());
        }
    }
}
