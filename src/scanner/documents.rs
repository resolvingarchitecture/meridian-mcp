use crate::models::{
    ArchitectureComponent, ArchitectureComponentType, ArchitectureContext, ArchitectureEvidence,
    ArchitectureModel, ArchitectureObservations, ArchitectureRelationship,
    ArchitectureRelationshipType, ContentEncoding, ContentType, DocumentContent, DocumentInput,
    DocumentTypeHint, Domain,
};
use crate::scanner::adrs;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Known architecture documentation files.
const ARCH_DOCS: &[&str] = &[
    "ARCHITECTURE.md",
    "architecture.md",
    "DESIGN.md",
    "design.md",
];

/// Maximum size for source/context files harvested directly into review documents.
///
/// The scanner should include architecture-significant evidence, but it should not
/// become a complete raw-source index or accidentally send large generated assets.
const MAX_ARCHITECTURE_SIGNIFICANT_FILE_BYTES: u64 = 256 * 1024;

/// Harvest all supported architecture/context documents from the project.
pub fn harvest(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut documents = Vec::new();

    documents.extend(adrs::harvest_from_paths(paths));
    documents.extend(harvest_architecture_docs(paths));
    documents.extend(harvest_architecture_significant_files(paths));

    documents
}

pub fn document_input_for_path(path: &Path, content: &str) -> Option<DocumentInput> {
    if !path.is_file() {
        return None;
    }

    if adrs::is_adr_path(path) {
        return Some(new_document_input(
            path,
            title_from_markdown(content).unwrap_or_else(|| "Untitled ADR".to_string()),
            DocumentTypeHint::ArchitectureDecisionRecord,
            Some("Architecture Decision Record discovered during local scan".to_string()),
            ContentType::Text,
            "text/plain",
            content.to_string(),
        ));
    }

    if is_architecture_doc_path(path) {
        let file_name = path.file_name()?.to_str()?;
        return Some(new_document_input(
            path,
            title_from_markdown(content).unwrap_or_else(|| file_name.to_string()),
            DocumentTypeHint::ApplicationDesign,
            Some("Architecture document discovered during local scan".to_string()),
            ContentType::Text,
            "text/plain",
            content.to_string(),
        ));
    }

    None
}

pub fn new_document_input(
    path: &Path,
    title: String,
    type_hint: DocumentTypeHint,
    stated_scope: Option<String>,
    content_type: ContentType,
    media_type: &str,
    data: String,
) -> DocumentInput {
    let content = new_document_content(content_type, media_type, data);
    let document_hash = aggregate_document_hash(std::slice::from_ref(&content));

    DocumentInput {
        id: document_id_for_path(path),
        title,
        filename: Some(path.to_string_lossy().to_string()),
        type_hint,
        author: None,
        date: None,
        version: None,
        stated_scope,
        organization_context: None,
        known_stakeholders: Vec::new(),
        known_decisions: Vec::new(),
        content: vec![content],
        data_hash: document_hash,
        data_hash_algorithm: "SHA-256".to_string(),
        scanned_at: Some(current_instant_string()),
    }
}

pub fn new_document_content(
    content_type: ContentType,
    media_type: &str,
    data: String,
) -> DocumentContent {
    DocumentContent {
        content_type,
        media_type: Some(media_type.to_string()),
        encoding: Some(ContentEncoding::Utf8),
        data_hash: content_hash(&data),
        data_hash_algorithm: "SHA-256".to_string(),
        data,
    }
}

pub fn aggregate_document_hash(content: &[DocumentContent]) -> String {
    let mut hasher = Sha256::new();

    for item in content {
        hasher.update(item.data_hash_algorithm.as_bytes());
        hasher.update(b":");
        hasher.update(item.data_hash.as_bytes());
        hasher.update(b"\n");
    }

    format!("{:x}", hasher.finalize())
}

pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn current_instant_string() -> String {
    humantime::format_rfc3339(std::time::SystemTime::now()).to_string()
}

pub fn title_from_markdown(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").trim().to_string())
}

fn harvest_architecture_docs(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut documents = Vec::new();

    for path in paths {
        if !is_architecture_doc_path(path) {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("architecture document");

            let title = title_from_markdown(&content).unwrap_or_else(|| file_name.to_string());
            let summary = format!("{file_name}: {title}");

            documents.push(new_document_input(
                path,
                title,
                DocumentTypeHint::ApplicationDesign,
                Some(summary),
                ContentType::Text,
                "text/plain",
                content,
            ));
        }
    }

    documents
}

fn harvest_architecture_significant_files(paths: &[PathBuf]) -> Vec<DocumentInput> {
    let mut documents = Vec::new();

    for path in paths {
        if !is_architecture_significant_file_path(path) {
            continue;
        }

        let Ok(metadata) = std::fs::metadata(path) else {
            continue;
        };

        if !metadata.is_file() || metadata.len() > MAX_ARCHITECTURE_SIGNIFICANT_FILE_BYTES {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("architecture-significant source");

        let Some(type_hint) = document_type_hint_for_architecture_significant_file(path) else {
            continue;
        };

        let media_type = media_type_for_path(path);
        let content_type = if matches!(
            type_hint,
            DocumentTypeHint::ApplicationDesign
                | DocumentTypeHint::ArchitectureDecisionRecord
                | DocumentTypeHint::InfrastructureDesign
                | DocumentTypeHint::SecurityDesign
                | DocumentTypeHint::ThreatModel
                | DocumentTypeHint::EnterpriseRoadmap
                | DocumentTypeHint::StandardsDocument
                | DocumentTypeHint::Runbook
        ) {
            ContentType::Text
        } else {
            ContentType::Code
        };

        documents.push(new_document_input(
            path,
            file_name.to_string(),
            type_hint,
            Some(format!(
                "Architecture-significant source discovered during local scan: {file_name}"
            )),
            content_type,
            media_type,
            content,
        ));
    }

    documents
}

fn is_architecture_doc_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    ARCH_DOCS.iter().any(|known| known == &file_name)
}

fn is_architecture_significant_file_path(path: &Path) -> bool {
    document_type_hint_for_architecture_significant_file(path).is_some()
}

fn document_type_hint_for_architecture_significant_file(path: &Path) -> Option<DocumentTypeHint> {
    if is_architecture_doc_path(path) || adrs::is_adr_path(path) {
        return None;
    }

    if is_runbook_path(path) {
        return Some(DocumentTypeHint::Runbook);
    }

    if is_threat_model_path(path) {
        return Some(DocumentTypeHint::ThreatModel);
    }

    if is_security_path(path) {
        return Some(DocumentTypeHint::SecurityDesign);
    }

    if is_infrastructure_path(path) {
        return Some(DocumentTypeHint::InfrastructureDesign);
    }

    if is_data_path(path) {
        return Some(DocumentTypeHint::DataModel);
    }

    if is_integration_path(path) {
        return Some(DocumentTypeHint::IntegrationDesign);
    }

    if is_application_source_path(path) {
        return Some(DocumentTypeHint::Codebase);
    }

    None
}

fn is_application_source_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    if matches_any(
        &file_name,
        &[
            "package.json",
            "tsconfig.json",
            "jsconfig.json",
            "vite.config.js",
            "vite.config.ts",
            "webpack.config.js",
            "webpack.config.ts",
            "rollup.config.js",
            "rollup.config.ts",
            "esbuild.config.js",
            "esbuild.config.ts",
            "next.config.js",
            "next.config.ts",
            "nuxt.config.js",
            "nuxt.config.ts",
            "pom.xml",
            "settings.gradle",
            "settings.gradle.kts",
            "build.gradle",
            "build.gradle.kts",
            "directory.build.props",
            "directory.build.targets",
            "global.json",
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "requirements.txt",
            "pipfile",
            "pipfile.lock",
            "poetry.lock",
            "tox.ini",
            "go.mod",
            "go.sum",
            "go.work",
            "cargo.toml",
            "cargo.lock",
            "build.rs",
            "cmakelists.txt",
            "makefile",
            "configure.ac",
            "meson.build",
            "gemfile",
            "gemfile.lock",
            "rakefile",
            "composer.json",
            "composer.lock",
            "package.swift",
            "podfile",
            "cartfile",
            "androidmanifest.xml",
            "info.plist",
            "entitlements.plist",
            "tauri.conf.json",
            "electron-builder.yml",
            "capacitor.config.json",
            "capacitor.config.ts",
            "pubspec.yaml",
            "pubspec.lock",
            "config.json",
            "config.yaml",
            "config.yml",
            "application.properties",
            "application.yml",
            "application.yaml",
            "bootstrap.yml",
            "bootstrap.yaml",
            "settings.py",
            "settings.toml",
            "config.toml",
            "config.edn",
            "config.exs",
        ],
    ) {
        return true;
    }

    if file_name.starts_with("tsconfig.") && file_name.ends_with(".json") {
        return true;
    }

    if file_name.starts_with("requirements-") && file_name.ends_with(".txt") {
        return true;
    }

    matches_extension(
        path,
        &[
            "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "java", "kt", "kts", "scala",
            "groovy", "gradle", "cs", "csproj", "sln", "fs", "fsproj", "vb", "vbproj", "py", "pyi",
            "go", "rs", "c", "h", "cc", "cpp", "cxx", "hpp", "hh", "hxx", "cmake", "rb", "rake",
            "gemspec", "php", "phtml", "swift", "m", "mm", "dart", "html", "htm", "css", "scss",
            "sass", "less", "vue", "svelte", "astro", "mdx", "feature", "robot", "bats",
        ],
    )
}

fn is_integration_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    matches_any(
        &file_name,
        &[
            "schema.graphql",
            "openapi.yaml",
            "openapi.yml",
            "openapi.json",
            "swagger.yaml",
            "swagger.yml",
            "swagger.json",
            "asyncapi.yaml",
            "asyncapi.yml",
            "asyncapi.json",
        ],
    ) || matches_extension(
        path,
        &[
            "proto",
            "thrift",
            "avdl",
            "avsc",
            "graphql",
            "gql",
            "jsonschema",
        ],
    )
}

fn is_data_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    let in_data_dir = path_contains_any_component(
        path,
        &[
            "migrations",
            "migration",
            "database",
            "schema",
            "schemas",
            "liquibase",
            "flyway",
            "seeds",
            "fixtures",
        ],
    );

    if in_data_dir {
        return matches_extension(
            path,
            &[
                "sql", "ddl", "dml", "xml", "rb", "py", "json", "yaml", "yml", "csv",
            ],
        );
    }

    if matches_any(
        &file_name,
        &[
            "changelog.xml",
            "liquibase.properties",
            "flyway.conf",
            "schema.rb",
            "structure.sql",
            "schema.prisma",
            "dbt_project.yml",
            "profiles.yml",
            "dvc.yaml",
            "dvc.lock",
            "pipeline.yaml",
            "pipeline.yml",
            "workflow.yaml",
            "workflow.yml",
            "kedro.yml",
            "catalog.yml",
            "catalog.yaml",
            "sources.yml",
            "sources.yaml",
            "metadata.yml",
            "metadata.yaml",
            "glossary.yml",
            "glossary.yaml",
            "seed.sql",
            "seeds.sql",
            "fixtures.yml",
            "fixtures.yaml",
        ],
    ) {
        return true;
    }

    if file_name.starts_with("db.changelog-") && file_name.ends_with(".xml") {
        return true;
    }

    file_name.ends_with(".schema.json")
        || file_name.ends_with(".schema.yaml")
        || file_name.ends_with(".schema.yml")
        || matches_extension(
            path,
            &[
                "sql",
                "ddl",
                "dml",
                "prisma",
                "cql",
                "ipynb",
                "r",
                "jl",
                "avsc",
                "avdl",
                "jsonschema",
            ],
        )
}

fn is_infrastructure_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    if path_contains_any_component(
        path,
        &[
            ".github",
            "workflows",
            ".circleci",
            ".buildkite",
            "tekton",
            "ops",
            "runbooks",
            "deploy",
            "deployment",
            "infrastructure",
            "templates",
            "group_vars",
            "host_vars",
        ],
    ) && matches_extension(path, &["yaml", "yml", "json", "toml", "conf", "tpl"])
    {
        return true;
    }

    if matches_any(
        &file_name,
        &[
            "dockerfile",
            ".dockerignore",
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
            "containerfile",
            "kustomization.yaml",
            "kustomization.yml",
            "chart.yaml",
            "values.yaml",
            ".terraform.lock.hcl",
            "terragrunt.hcl",
            "terragrunt.hcl.json",
            "template.yaml",
            "template.yml",
            "samconfig.toml",
            "pulumi.yaml",
            "azure-pipelines.yml",
            "azure-pipelines.yaml",
            "cloudbuild.yaml",
            "cloudbuild.yml",
            "firebase.json",
            ".firebaserc",
            ".gitlab-ci.yml",
            "jenkinsfile",
            "bitbucket-pipelines.yml",
            "buildkite.yml",
            "drone.yml",
            "drone.yaml",
            "playbook.yml",
            "playbook.yaml",
            "inventory",
            "inventory.ini",
            "packer.json",
            "vagrantfile",
            "serverless.yml",
            "serverless.yaml",
            "sst.config.ts",
            "sst.config.js",
            "netlify.toml",
            "vercel.json",
            "wrangler.toml",
            "wrangler.json",
            "app.yaml",
            "dispatch.yaml",
            "prometheus.yml",
            "prometheus.yaml",
            "alertmanager.yml",
            "alertmanager.yaml",
            "otel-collector-config.yaml",
            "datadog.yaml",
            "newrelic.yml",
        ],
    ) {
        return true;
    }

    file_name.starts_with("dockerfile.")
        || file_name.starts_with("jenkinsfile.")
        || file_name.starts_with("values.") && file_name.ends_with(".yaml")
        || file_name.starts_with("pulumi.")
            && (file_name.ends_with(".yaml") || file_name.ends_with(".json"))
        || matches_extension(
            path,
            &[
                "tf", "tfvars", "bicep", "jinja", "pkr.hcl", "pp", "sls", "conf", "toml",
            ],
        )
}

fn is_security_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    if path_contains_any_component(path, &["security", "compliance", "risk"]) {
        return matches_extension(
            path,
            &[
                "md",
                "adoc",
                "rst",
                "txt",
                "yaml",
                "yml",
                "json",
                "csv",
                "rego",
                "sentinel",
                "cedar",
                "tf",
                "conf",
                "toml",
                "properties",
            ],
        );
    }

    if matches_any(
        &file_name,
        &[
            "security.md",
            "risk.md",
            "compliance.md",
            "privacy.md",
            "data_protection.md",
            "data-protection.md",
            "rbac.yaml",
            "rbac.yml",
            ".env.example",
            ".env.sample",
            ".env.template",
            "secrets.example.yml",
            "secrets.example.yaml",
            ".sops.yaml",
            "package-lock.json",
            "npm-shrinkwrap.json",
            "yarn.lock",
            "pnpm-lock.yaml",
            "cargo.lock",
            "go.sum",
            "gemfile.lock",
            "poetry.lock",
            "pipfile.lock",
            "composer.lock",
            "gradle.lockfile",
            "verification-metadata.xml",
            "cosign.pub",
            ".semgrep.yml",
            ".semgrep.yaml",
            "semgrep.yml",
            "semgrep.yaml",
            ".snyk",
            "snyk.yml",
            "snyk.yaml",
            "trivy.yaml",
            "trivy.yml",
            "grype.yaml",
            "grype.yml",
            "gitleaks.toml",
            ".gitleaks.toml",
            "detect-secrets.yaml",
            ".detect-secrets.baseline",
            "codeql-config.yml",
            "codeql-config.yaml",
        ],
    ) {
        return true;
    }

    file_name.starts_with("slsa") && file_name.ends_with(".json")
        || matches_extension(path, &["rego", "sentinel", "cedar"])
}

fn is_threat_model_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    matches_any(
        &file_name,
        &[
            "threat_model.md",
            "threat-model.md",
            "threat_model.yaml",
            "threat-model.yaml",
        ],
    )
}

fn is_runbook_path(path: &Path) -> bool {
    let Some(file_name) = normalized_file_name(path) else {
        return false;
    };

    matches_any(
        &file_name,
        &[
            "runbook.md",
            "operations.md",
            "sre.md",
            "oncall.md",
            "incident-response.md",
            "playbook.md",
        ],
    ) || path_contains_any_component(path, &["ops", "runbooks"])
        && matches_extension(path, &["md", "adoc", "rst", "txt"])
}

fn matches_extension(path: &Path, extensions: &[&str]) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };

    let extension = extension.to_ascii_lowercase();
    extensions.iter().any(|known| known == &extension)
}

fn normalized_file_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
}

fn matches_any(value: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| value == *candidate)
}

fn path_contains_any_component(path: &Path, components: &[&str]) -> bool {
    path.components().any(|component| {
        let component = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        components.iter().any(|known| component == *known)
    })
}

fn document_id_for_path(path: &Path) -> String {
    let normalized = path.to_string_lossy();
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
        "document".to_string()
    } else {
        format!("document-{trimmed}")
    }
}

/// Build a local architecture model from already-harvested documents.
///
/// This intentionally runs on the client/MCP side so raw source does not need to
/// be sent to the backend for architecture discovery. The backend can then focus
/// on readiness/review validation against this structural model.
pub fn discover_architecture_model(
    context_id: Uuid,
    documents: &[DocumentInput],
) -> ArchitectureModel {
    let evidence = documents
        .iter()
        .map(architecture_evidence_for_document)
        .collect::<Vec<_>>();

    let layers = infer_layers(documents);
    let layer_order = infer_layer_order(&layers);
    let patterns = detect_patterns(documents);
    let adrs = harvest_adr_refs(documents);
    let modules = infer_modules(documents);
    let technologies = infer_technologies(documents);
    let conventions = infer_conventions(documents);
    let risks = infer_risks(documents);

    let global_observations = ArchitectureObservations {
        layers: layers.clone(),
        layer_order: layer_order.clone(),
        style: infer_style(&layers, &patterns),
        patterns: patterns.clone(),
        adrs: adrs.clone(),
        modules: modules.clone(),
        technologies: technologies.clone(),
        risks,
        conventions,
    };

    let components = discover_components(documents, &layers, &layer_order, &patterns, &adrs);

    let relationships = discover_relationships(&components, documents);

    let mut warnings = Vec::new();
    if components.is_empty() {
        warnings.push(
            "No architecture boundaries were discovered from local project files.".to_string(),
        );
    }

    if !documents.iter().any(is_source_document) {
        warnings.push(
            "No local source-code documents were available for source-based architecture discovery."
                .to_string(),
        );
    }

    if !documents.iter().any(is_design_document) {
        warnings.push(
            "No local design documents were available for design-intent architecture discovery."
                .to_string(),
        );
    }

    let context =
        discover_architecture_context(context_id, documents, &components, &global_observations);

    ArchitectureModel {
        context_id: Some(context_id),
        context,
        components,
        relationships,
        global_observations,
        evidence,
        warnings,
    }
}

fn architecture_evidence_for_document(document: &DocumentInput) -> ArchitectureEvidence {
    ArchitectureEvidence {
        evidence_id: evidence_id_for_document(document),
        source_type: format!("{:?}", document.type_hint),
        path: document.filename.clone(),
        description: document
            .stated_scope
            .clone()
            .unwrap_or_else(|| document.title.clone()),
        scanned_at: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
        ),
    }
}

fn discover_components(
    documents: &[DocumentInput],
    layers: &[String],
    layer_order: &[String],
    patterns: &[String],
    adrs: &[String],
) -> Vec<ArchitectureComponent> {
    let mut components_by_id = BTreeMap::new();

    for document in documents {
        let Some((domain, component_type, boundary)) = component_seed_for_document(document) else {
            continue;
        };

        let component_id = format!(
            "{}-{}",
            domain_name(&domain).to_ascii_lowercase(),
            normalize_token(&boundary)
        );

        let observations = ArchitectureObservations {
            layers: layers_for_document(document, layers),
            layer_order: layer_order_for_document(document, layer_order),
            style: infer_style(layers, patterns),
            patterns: patterns_for_document(document, patterns),
            adrs: adrs.to_vec(),
            modules: modules_for_document(document),
            technologies: technologies_for_document(document),
            risks: risks_for_document(document),
            conventions: conventions_for_document(document),
        };

        let component = ArchitectureComponent {
            component_id: component_id.clone(),
            name: display_boundary_name(&boundary),
            component_type,
            domain,
            root: Some(boundary),
            language: infer_language(document),
            framework: infer_framework(document),
            observations,
            evidence_refs: vec![evidence_id_for_document(document)],
            confidence: confidence_for_document(document),
        };

        components_by_id
            .entry(component_id)
            .and_modify(|existing: &mut ArchitectureComponent| {
                merge_component(existing, &component);
            })
            .or_insert(component);
    }

    components_by_id.into_values().collect()
}

fn component_seed_for_document(
    document: &DocumentInput,
) -> Option<(Domain, ArchitectureComponentType, String)> {
    match document.type_hint {
        DocumentTypeHint::ArchitectureDecisionRecord => Some((
            Domain::Application,
            ArchitectureComponentType::Module,
            boundary_for_document(document),
        )),
        DocumentTypeHint::ApplicationDesign | DocumentTypeHint::Codebase => Some((
            Domain::Application,
            ArchitectureComponentType::Application,
            application_boundary_for_document(document),
        )),
        DocumentTypeHint::IntegrationDesign => Some((
            Domain::Integration,
            ArchitectureComponentType::Integration,
            integration_boundary_for_document(document),
        )),
        DocumentTypeHint::DataModel => Some((
            Domain::Data,
            ArchitectureComponentType::DataStore,
            data_boundary_for_document(document),
        )),
        DocumentTypeHint::InfrastructureDesign | DocumentTypeHint::Runbook => Some((
            Domain::Infrastructure,
            ArchitectureComponentType::Infrastructure,
            infrastructure_boundary_for_document(document),
        )),
        DocumentTypeHint::SecurityDesign | DocumentTypeHint::ThreatModel => Some((
            Domain::Security,
            ArchitectureComponentType::SecurityScope,
            "security-architecture".to_string(),
        )),
        DocumentTypeHint::EnterpriseRoadmap | DocumentTypeHint::StandardsDocument => Some((
            Domain::Enterprise,
            ArchitectureComponentType::EnterpriseScope,
            "enterprise-architecture".to_string(),
        )),
        DocumentTypeHint::Other => None,
    }
}

fn discover_relationships(
    components: &[ArchitectureComponent],
    documents: &[DocumentInput],
) -> Vec<ArchitectureRelationship> {
    let mut relationships = Vec::new();

    for source in components {
        for target in components {
            if source.component_id == target.component_id {
                continue;
            }

            if relationship_exists(source, target, documents) {
                relationships.push(ArchitectureRelationship {
                    source_component_id: source.component_id.clone(),
                    target_component_id: target.component_id.clone(),
                    relationship_type: ArchitectureRelationshipType::DependsOn,
                    protocol: infer_relationship_protocol(source, target, documents),
                    evidence_refs: shared_or_source_evidence_refs(source, target),
                    confidence: 0.60,
                });
            }
        }
    }

    relationships
}

fn relationship_exists(
    source: &ArchitectureComponent,
    target: &ArchitectureComponent,
    documents: &[DocumentInput],
) -> bool {
    if source.domain == target.domain {
        return false;
    }

    let target_name = target.name.to_ascii_lowercase();
    let target_root = target.root.clone().unwrap_or_default().to_ascii_lowercase();

    documents
        .iter()
        .filter(|document| {
            let evidence_id = evidence_id_for_document(document);
            source.evidence_refs.contains(&evidence_id)
        })
        .map(searchable_text)
        .any(|text| {
            (!target_name.is_empty() && text.contains(&target_name))
                || (!target_root.is_empty() && text.contains(&target_root))
                || relationship_keyword_match(&text, target.domain.clone())
        })
}

fn relationship_keyword_match(text: &str, domain: Domain) -> bool {
    match domain {
        Domain::Integration => contains_any(
            text,
            &[
                "api", "rest", "grpc", "webhook", "event", "queue", "topic", "mcp", "stripe",
                "bitcoin", "bedrock",
            ],
        ),
        Domain::Data => contains_any(
            text,
            &[
                "database",
                "schema",
                "table",
                "repository",
                "postgres",
                "mysql",
                "sqlite",
                "redis",
            ],
        ),
        Domain::Infrastructure => contains_any(
            text,
            &[
                "docker",
                "kubernetes",
                "terraform",
                "deployment",
                "cloud",
                "runtime",
                "container",
            ],
        ),
        Domain::Security => contains_any(
            text,
            &[
                "auth",
                "authentication",
                "authorization",
                "security",
                "iam",
                "oauth",
                "jwt",
                "encryption",
            ],
        ),
        Domain::Application | Domain::Enterprise => false,
    }
}

fn infer_relationship_protocol(
    _source: &ArchitectureComponent,
    _target: &ArchitectureComponent,
    documents: &[DocumentInput],
) -> Option<String> {
    let combined = documents
        .iter()
        .map(searchable_text)
        .collect::<Vec<_>>()
        .join("\n");

    if contains_any(&combined, &["grpc", "protobuf"]) {
        Some("gRPC".to_string())
    } else if contains_any(&combined, &["rest", "http", "openapi", "swagger"]) {
        Some("HTTP/REST".to_string())
    } else if contains_any(&combined, &["queue", "topic", "event", "kafka", "rabbitmq"]) {
        Some("messaging".to_string())
    } else {
        None
    }
}

fn discover_architecture_context(
    context_id: Uuid,
    documents: &[DocumentInput],
    components: &[ArchitectureComponent],
    observations: &ArchitectureObservations,
) -> ArchitectureContext {
    let scope_notes = {
        let mut notes = Vec::new();

        if !components.is_empty() {
            notes.push(format!(
                "Discovered architecture components locally: {:?}",
                components
                    .iter()
                    .map(|component| component.name.clone())
                    .collect::<Vec<_>>()
            ));
        }

        if !observations.layers.is_empty() {
            notes.push(format!("Discovered layers: {:?}", observations.layers));
        }

        if !observations.patterns.is_empty() {
            notes.push(format!(
                "Discovered architecture/code patterns: {:?}",
                observations.patterns
            ));
        }

        notes.extend(
            documents
                .iter()
                .filter_map(|document| document.stated_scope.clone())
                .filter(|scope| !scope.trim().is_empty()),
        );

        if notes.is_empty() {
            None
        } else {
            Some(unique(notes))
        }
    };

    let freeform_notes = if components.is_empty() {
        None
    } else {
        Some(
            "Architecture context was discovered locally by meridian-mcp from project files. Raw source content is not required by the backend for model discovery."
                .to_string(),
        )
    };

    ArchitectureContext {
        context_id: Some(context_id),
        organization_context: documents
            .iter()
            .find_map(|document| document.organization_context.clone()),
        business_goals: None,
        stakeholders: Some(
            documents
                .iter()
                .flat_map(|document| document.known_stakeholders.clone())
                .collect(),
        ),
        decisions: Some(
            documents
                .iter()
                .flat_map(|document| document.known_decisions.clone())
                .collect(),
        ),
        constraints: None,
        risks: Some(observations.risks.clone()),
        standards: Some(observations.conventions.clone()),
        scope_notes,
        freeform_notes,
    }
}

fn infer_layers(documents: &[DocumentInput]) -> Vec<String> {
    let mut layers = BTreeSet::new();

    for document in documents {
        let path = document_path(document)
            .replace('\\', "/")
            .to_ascii_lowercase();
        let text = searchable_text(document);

        for candidate in [
            "domain",
            "application",
            "usecase",
            "usecases",
            "service",
            "services",
            "api",
            "web",
            "controller",
            "controllers",
            "ports",
            "adapters",
            "adapter",
            "infrastructure",
            "infra",
            "persistence",
            "repository",
            "repositories",
            "dao",
        ] {
            if path.contains(candidate) || text.contains(candidate) {
                layers.insert(canonical_layer(candidate).to_string());
            }
        }
    }

    layers.into_iter().collect()
}

fn infer_layer_order(layers: &[String]) -> Vec<String> {
    let preferred = [
        "api",
        "application",
        "service",
        "domain",
        "ports",
        "adapters",
        "persistence",
        "infrastructure",
    ];

    preferred
        .iter()
        .filter(|layer| layers.iter().any(|existing| existing == **layer))
        .map(|layer| layer.to_string())
        .collect()
}

fn detect_patterns(documents: &[DocumentInput]) -> Vec<String> {
    let combined = documents
        .iter()
        .filter(|document| is_source_document(document) || is_design_document(document))
        .take(80)
        .map(searchable_text)
        .collect::<Vec<_>>()
        .join("\n");

    let mut patterns = BTreeSet::new();

    if contains_any(&combined, &["repository", "interface ", "implements"]) {
        patterns.insert("repository_pattern".to_string());
    }

    if contains_any(&combined, &["constructor(private", "private final"]) {
        patterns.insert("constructor_injection".to_string());
    }

    if contains_any(&combined, &["value object", "valueobject", "record "]) {
        patterns.insert("value_objects".to_string());
    }

    if contains_any(&combined, &["dto", "request", "response"]) {
        patterns.insert("dto_boundary".to_string());
    }

    if contains_any(&combined, &["command", "query", "handler"]) {
        patterns.insert("cqrs".to_string());
    }

    if contains_any(&combined, &["event", "publish", "dispatch", "emit"]) {
        patterns.insert("domain_events".to_string());
    }

    if contains_any(&combined, &["hexagonal", "ports and adapters"]) {
        patterns.insert("hexagonal_architecture".to_string());
    }

    if contains_any(&combined, &["clean architecture"]) {
        patterns.insert("clean_architecture".to_string());
    }

    if contains_any(&combined, &["microservice", "microservices"]) {
        patterns.insert("microservices".to_string());
    }

    patterns.into_iter().collect()
}

fn harvest_adr_refs(documents: &[DocumentInput]) -> Vec<String> {
    documents
        .iter()
        .filter(|document| {
            matches!(
                document.type_hint,
                DocumentTypeHint::ArchitectureDecisionRecord
            )
        })
        .map(|document| {
            format!(
                "{}: {}",
                document
                    .filename
                    .clone()
                    .unwrap_or_else(|| document.id.clone()),
                document.title
            )
        })
        .collect()
}

fn infer_modules(documents: &[DocumentInput]) -> Vec<String> {
    unique(
        documents
            .iter()
            .flat_map(modules_for_document)
            .collect::<Vec<_>>(),
    )
}

fn infer_technologies(documents: &[DocumentInput]) -> Vec<String> {
    unique(
        documents
            .iter()
            .flat_map(technologies_for_document)
            .collect::<Vec<_>>(),
    )
}

fn infer_conventions(documents: &[DocumentInput]) -> Vec<String> {
    unique(
        documents
            .iter()
            .flat_map(conventions_for_document)
            .collect::<Vec<_>>(),
    )
}

fn infer_risks(documents: &[DocumentInput]) -> Vec<String> {
    unique(
        documents
            .iter()
            .flat_map(risks_for_document)
            .collect::<Vec<_>>(),
    )
}

fn layers_for_document(document: &DocumentInput, all_layers: &[String]) -> Vec<String> {
    let text = searchable_text(document);
    all_layers
        .iter()
        .filter(|layer| text.contains(layer.as_str()))
        .cloned()
        .collect()
}

fn layer_order_for_document(
    document: &DocumentInput,
    global_layer_order: &[String],
) -> Vec<String> {
    let document_layers = layers_for_document(document, global_layer_order);

    global_layer_order
        .iter()
        .filter(|layer| document_layers.contains(layer))
        .cloned()
        .collect()
}

fn patterns_for_document(document: &DocumentInput, global_patterns: &[String]) -> Vec<String> {
    let text = searchable_text(document);

    global_patterns
        .iter()
        .filter(|pattern| text.contains(pattern.as_str()) || pattern_is_global_signal(pattern))
        .cloned()
        .collect()
}

fn pattern_is_global_signal(pattern: &str) -> bool {
    matches!(
        pattern,
        "repository_pattern"
            | "constructor_injection"
            | "dto_boundary"
            | "hexagonal_architecture"
            | "clean_architecture"
            | "microservices"
    )
}

fn modules_for_document(document: &DocumentInput) -> Vec<String> {
    document_path(document)
        .replace('\\', "/")
        .split('/')
        .map(normalize_token)
        .filter(|part| {
            !part.is_empty()
                && !matches!(
                    part.as_str(),
                    "src" | "main" | "test" | "java" | "resources" | "target" | "node-modules"
                )
        })
        .collect()
}

fn technologies_for_document(document: &DocumentInput) -> Vec<String> {
    let mut technologies = BTreeSet::new();
    let text = searchable_text(document);

    if let Some(language) = infer_language(document) {
        technologies.insert(language);
    }

    for (needle, technology) in [
        ("springframework", "spring"),
        ("@springbootapplication", "spring_boot"),
        ("react", "react"),
        ("next", "nextjs"),
        ("express", "express"),
        ("tokio", "tokio"),
        ("serde", "serde"),
        ("postgres", "postgres"),
        ("mysql", "mysql"),
        ("redis", "redis"),
        ("docker", "docker"),
        ("kubernetes", "kubernetes"),
        ("terraform", "terraform"),
    ] {
        if text.contains(needle) {
            technologies.insert(technology.to_string());
        }
    }

    technologies.into_iter().collect()
}

fn conventions_for_document(document: &DocumentInput) -> Vec<String> {
    let text = searchable_text(document);
    let mut conventions = BTreeSet::new();

    if text.contains("src/main/java") {
        conventions.insert("maven_or_gradle_standard_layout".to_string());
    }

    if text.contains("src/test/java") {
        conventions.insert("java_test_layout".to_string());
    }

    if text.contains("package ") {
        conventions.insert("package_namespaces".to_string());
    }

    if contains_any(&text, &["@service", "@repository", "@restcontroller"]) {
        conventions.insert("spring_stereotypes".to_string());
    }

    conventions.into_iter().collect()
}

fn risks_for_document(document: &DocumentInput) -> Vec<String> {
    content_text(document)
        .lines()
        .map(str::trim)
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            contains_any(
                &lower,
                &[
                    "risk",
                    "threat",
                    "trade-off",
                    "tradeoff",
                    "assumption",
                    "constraint",
                    "technical debt",
                    "vulnerability",
                ],
            )
        })
        .map(cleanup_context_line)
        .filter(|line| !line.is_empty())
        .take(25)
        .collect()
}

fn infer_style(layers: &[String], patterns: &[String]) -> Option<String> {
    let has_domain = layers.iter().any(|layer| layer == "domain");
    let has_ports = layers
        .iter()
        .any(|layer| layer == "ports" || layer == "adapters");
    let has_application = layers.iter().any(|layer| layer == "application");
    let has_repository = patterns
        .iter()
        .any(|pattern| pattern == "repository_pattern");

    if has_domain && has_ports {
        Some("hexagonal".to_string())
    } else if has_domain && has_application {
        Some("clean_architecture".to_string())
    } else if has_domain && has_repository {
        Some("layered_ddd".to_string())
    } else if has_domain {
        Some("layered".to_string())
    } else if !layers.is_empty() || !patterns.is_empty() {
        Some("modular".to_string())
    } else {
        None
    }
}

fn application_boundary_for_document(document: &DocumentInput) -> String {
    let path = document_path(document).replace('\\', "/");

    for marker in ["/src/main/", "/src/", "/app/", "/server/"] {
        if let Some(index) = path.find(marker) {
            return path[..index].to_string();
        }
    }

    boundary_for_document(document)
}

fn integration_boundary_for_document(document: &DocumentInput) -> String {
    let text = searchable_text(document);

    if text.contains("mcp") {
        "mcp-integration".to_string()
    } else if text.contains("bedrock") {
        "bedrock-integration".to_string()
    } else if contains_any(&text, &["bitcoin", "lightning"]) {
        "bitcoin-integration".to_string()
    } else if text.contains("stripe") {
        "stripe-integration".to_string()
    } else if contains_any(&text, &["queue", "topic", "event", "channel"]) {
        "event-integration".to_string()
    } else {
        "external-integration".to_string()
    }
}

fn data_boundary_for_document(document: &DocumentInput) -> String {
    let text = searchable_text(document);

    if contains_any(&text, &["postgres", "postgresql"]) {
        "postgres-operational-data-store".to_string()
    } else if contains_any(&text, &["mysql", "mariadb"]) {
        "mysql-operational-data-store".to_string()
    } else if text.contains("redis") {
        "redis-data-store".to_string()
    } else if contains_any(&text, &["migration", "create table", "schema"]) {
        "relational-operational-data-store".to_string()
    } else {
        "application-data-store".to_string()
    }
}

fn infrastructure_boundary_for_document(document: &DocumentInput) -> String {
    let text = searchable_text(document);

    if contains_any(&text, &["docker", "dockerfile", "container"]) {
        "container-deployment-model".to_string()
    } else if contains_any(&text, &["kubernetes", "helm", "pod", "ingress"]) {
        "kubernetes-deployment-model".to_string()
    } else if contains_any(&text, &["terraform", "vpc", "aws", "cloud"]) {
        "cloud-deployment-model".to_string()
    } else if contains_any(&text, &["ci/cd", "pipeline", "github actions"]) {
        "delivery-pipeline".to_string()
    } else {
        "runtime-deployment-model".to_string()
    }
}

fn boundary_for_document(document: &DocumentInput) -> String {
    document
        .filename
        .as_deref()
        .and_then(|filename| {
            let path = Path::new(filename);
            path.parent()
                .map(|parent| parent.to_string_lossy().to_string())
        })
        .filter(|value| !value.trim().is_empty())
        .or_else(|| document.stated_scope.clone())
        .unwrap_or_else(|| document.title.clone())
}

fn display_boundary_name(boundary: &str) -> String {
    let last = boundary
        .replace('\\', "/")
        .split('/')
        .last()
        .unwrap_or(boundary)
        .replace(['-', '_'], " ");

    let name = last.trim();

    if name.is_empty() {
        "Architecture Boundary".to_string()
    } else {
        name.split_whitespace()
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn confidence_for_document(document: &DocumentInput) -> f64 {
    let mut confidence: f64 = if is_design_document(document) {
        0.75
    } else {
        0.60
    };

    if is_source_document(document) {
        confidence += 0.05;
    }

    if document
        .stated_scope
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        confidence += 0.05;
    }

    if !document.known_decisions.is_empty() || !document.known_stakeholders.is_empty() {
        confidence += 0.05;
    }

    confidence.min(0.95)
}

fn merge_component(existing: &mut ArchitectureComponent, next: &ArchitectureComponent) {
    for evidence_ref in &next.evidence_refs {
        if !existing.evidence_refs.contains(evidence_ref) {
            existing.evidence_refs.push(evidence_ref.clone());
        }
    }

    existing.confidence = existing.confidence.max(next.confidence);

    existing.observations.layers = unique(
        existing
            .observations
            .layers
            .iter()
            .chain(next.observations.layers.iter())
            .cloned()
            .collect(),
    );

    existing.observations.patterns = unique(
        existing
            .observations
            .patterns
            .iter()
            .chain(next.observations.patterns.iter())
            .cloned()
            .collect(),
    );

    existing.observations.modules = unique(
        existing
            .observations
            .modules
            .iter()
            .chain(next.observations.modules.iter())
            .cloned()
            .collect(),
    );

    existing.observations.technologies = unique(
        existing
            .observations
            .technologies
            .iter()
            .chain(next.observations.technologies.iter())
            .cloned()
            .collect(),
    );
}

fn shared_or_source_evidence_refs(
    source: &ArchitectureComponent,
    target: &ArchitectureComponent,
) -> Vec<String> {
    let mut refs = source.evidence_refs.clone();

    for evidence_ref in &target.evidence_refs {
        if !refs.contains(evidence_ref) {
            refs.push(evidence_ref.clone());
        }
    }

    refs
}

fn is_design_document(document: &DocumentInput) -> bool {
    matches!(
        document.type_hint,
        DocumentTypeHint::ArchitectureDecisionRecord
            | DocumentTypeHint::ApplicationDesign
            | DocumentTypeHint::IntegrationDesign
            | DocumentTypeHint::DataModel
            | DocumentTypeHint::InfrastructureDesign
            | DocumentTypeHint::SecurityDesign
            | DocumentTypeHint::ThreatModel
            | DocumentTypeHint::EnterpriseRoadmap
            | DocumentTypeHint::StandardsDocument
            | DocumentTypeHint::Runbook
    )
}

fn is_source_document(document: &DocumentInput) -> bool {
    matches!(document.type_hint, DocumentTypeHint::Codebase)
        || document
            .content
            .iter()
            .any(|content| matches!(content.content_type, ContentType::Code))
}

fn infer_language(document: &DocumentInput) -> Option<String> {
    let path = document_path(document).to_ascii_lowercase();
    let text = searchable_text(document);

    if path.ends_with(".java") || text.contains("public class ") || text.contains("public record ")
    {
        Some("java".to_string())
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        Some("typescript".to_string())
    } else if path.ends_with(".js")
        || path.ends_with(".jsx")
        || path.ends_with(".mjs")
        || path.ends_with(".cjs")
    {
        Some("javascript".to_string())
    } else if path.ends_with(".py") {
        Some("python".to_string())
    } else if path.ends_with(".rs") {
        Some("rust".to_string())
    } else if path.ends_with(".go") {
        Some("go".to_string())
    } else if path.ends_with(".rb") {
        Some("ruby".to_string())
    } else if path.ends_with(".php") {
        Some("php".to_string())
    } else if path.ends_with(".cs") {
        Some("csharp".to_string())
    } else {
        None
    }
}

fn infer_framework(document: &DocumentInput) -> Option<String> {
    let text = searchable_text(document);

    if text.contains("springframework") || text.contains("@springbootapplication") {
        Some("spring".to_string())
    } else if text.contains("react") {
        Some("react".to_string())
    } else if text.contains("next") {
        Some("nextjs".to_string())
    } else if text.contains("express") {
        Some("express".to_string())
    } else if text.contains("tokio") {
        Some("tokio".to_string())
    } else {
        None
    }
}

fn document_path(document: &DocumentInput) -> String {
    document
        .filename
        .clone()
        .unwrap_or_else(|| document.title.clone())
}

fn searchable_text(document: &DocumentInput) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        document.id,
        document.title,
        document.filename.clone().unwrap_or_default(),
        document.stated_scope.clone().unwrap_or_default(),
        content_text(document)
    )
    .to_ascii_lowercase()
}

fn content_text(document: &DocumentInput) -> String {
    document
        .content
        .iter()
        .map(|content| content.data.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn evidence_id_for_document(document: &DocumentInput) -> String {
    format!("evidence-{}", normalize_token(&document.id))
}

fn normalize_token(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug
    }
}

fn canonical_layer(layer: &str) -> &str {
    match layer {
        "usecase" | "usecases" => "application",
        "services" => "service",
        "web" | "controller" | "controllers" => "api",
        "adapter" => "adapters",
        "infra" => "infrastructure",
        "repository" | "repositories" | "dao" => "persistence",
        value => value,
    }
}

fn domain_name(domain: &Domain) -> &'static str {
    match domain {
        Domain::Application => "application",
        Domain::Integration => "integration",
        Domain::Data => "data",
        Domain::Infrastructure => "infrastructure",
        Domain::Security => "security",
        Domain::Enterprise => "enterprise",
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn cleanup_context_line(line: &str) -> String {
    line.trim()
        .trim_start_matches("- ")
        .trim_start_matches("* ")
        .trim_start_matches("# ")
        .trim()
        .to_string()
}

fn unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn media_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => "application/json",
        Some("yaml") | Some("yml") => "application/yaml",
        Some("toml") => "application/toml",
        Some("xml") => "application/xml",
        Some("sql") | Some("ddl") | Some("dml") => "application/sql",
        Some("md") | Some("mdx") => "text/markdown",
        Some("html") | Some("htm") => "text/html",
        Some("css") | Some("scss") | Some("sass") | Some("less") => "text/css",
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => "text/javascript",
        Some("ts") | Some("tsx") | Some("mts") | Some("cts") => "text/typescript",
        Some("java") => "text/x-java-source",
        Some("kt") | Some("kts") => "text/x-kotlin",
        Some("rs") => "text/x-rust",
        Some("go") => "text/x-go",
        Some("py") | Some("pyi") => "text/x-python",
        Some("rb") | Some("rake") => "text/x-ruby",
        Some("php") | Some("phtml") => "text/x-php",
        Some("cs") => "text/x-csharp",
        Some("c") | Some("h") | Some("cc") | Some("cpp") | Some("cxx") | Some("hpp")
        | Some("hh") | Some("hxx") => "text/x-c",
        Some("proto") => "text/x-protobuf",
        Some("graphql") | Some("gql") => "application/graphql",
        Some("tf") | Some("tfvars") => "text/x-terraform",
        Some("sh") | Some("bash") | Some("zsh") | Some("fish") => "text/x-shellscript",
        _ => "text/plain",
    }
}
