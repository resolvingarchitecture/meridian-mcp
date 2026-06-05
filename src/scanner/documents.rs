use crate::models::{
    ContentEncoding, ContentType, DocumentContent, DocumentInput, DocumentTypeHint,
};
use crate::scanner::adrs;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
