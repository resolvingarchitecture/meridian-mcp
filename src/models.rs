use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureContext {
    pub context_id: Option<Uuid>,
    pub organization_context: Option<serde_json::Value>,
    pub business_goals: Option<Vec<String>>,
    pub stakeholders: Option<Vec<serde_json::Value>>,
    pub decisions: Option<Vec<serde_json::Value>>,
    pub constraints: Option<Vec<String>>,
    pub risks: Option<Vec<String>>,
    pub standards: Option<Vec<String>>,
    pub scope_notes: Option<Vec<String>>,
    pub freeform_notes: Option<String>,
}

impl ArchitectureContext {
    pub fn new() -> Self {
        Self::new_with_context_id(Uuid::new_v4())
    }

    pub fn new_with_context_id(context_id: Uuid) -> Self {
        Self {
            context_id: Some(context_id),
            organization_context: None,
            business_goals: None,
            stakeholders: None,
            decisions: None,
            constraints: None,
            risks: None,
            standards: None,
            scope_notes: None,
            freeform_notes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: String,
    #[serde(rename = "type")]
    pub finding_type: String,
    pub file: String,
    pub line: Option<u32>,
    pub title: String,
    pub explanation: String,
    pub consequence: String,
    pub suggestion: String,
    pub adr_reference: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainReadiness {
    pub rubric: String,
    pub detected: bool,
    pub domain: Domain,
    pub present: bool,
    pub estimated_num_components: i32,
    pub complexity_modifier: ComplexityModifier,
    pub rationale: String,
    pub confidence: f64,
    pub sufficient_for_high_fidelity_review: bool,
    pub supporting_evidence: Vec<String>,
    pub missing_context: Vec<String>,
    pub warnings: Vec<String>,
    pub review_targets: Vec<ReviewTargetReadiness>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTargetReadiness {
    pub target_id: String,
    pub target_name: String,
    pub domain: Domain,
    pub target_type: String,
    pub complexity_modifier: ComplexityModifier,
    pub confidence: f64,
    pub sufficient_for_high_fidelity_review: bool,
    pub supporting_evidence: Vec<String>,
    pub missing_context: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureReviewReadiness {
    pub context_id: String,
    pub status: String,
    pub question: String,
    pub domain_readiness_list: Vec<DomainReadiness>,
    pub architecture_context: Option<ArchitectureContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthHeartbeat {
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthNResult {
    pub session_id: String,
    pub expires_at: u64,
    pub status: Option<i32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    pub phone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestApiKeyRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestBitcoinFundingRequest {
    /// Number of sats to buy/fund into the authenticated Meridian account.
    pub amount_sats: u64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinFundingStatusRequest {
    /// Bitcoin receive address returned by request_bitcoin_funding.
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinFundingRequestResponse {
    pub address: String,
    pub amount_sats: u64,
    pub amount_usd: String,
    pub exchange_rate: String,
    pub expires_at: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinFundingStatusResponse {
    pub address: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComplexityModifier {
    Simple,
    Moderate,
    Complex,
    VeryComplex,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddContextRequest {
    #[schemars(with = "Option<String>")]
    pub context_id: Option<uuid::Uuid>,
    pub organization_context: Option<serde_json::Value>,
    pub business_goals: Option<Vec<String>>,
    pub stakeholders: Option<Vec<serde_json::Value>>,
    pub decisions: Option<Vec<serde_json::Value>>,
    pub constraints: Option<Vec<String>>,
    pub risks: Option<Vec<String>>,
    pub standards: Option<Vec<String>>,
    pub scope_notes: Option<Vec<String>>,
    pub freeform_notes: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanProjectRequest {
    pub root_dir: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReviewRequest {}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
    Renamed,
    Moved,
    Unknown,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EvaluateDocumentChangeRequest {
    pub file_path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct DocumentChangeEvaluation {
    pub requires_intermediate_review: bool,
    pub reason: String,
    pub updated_document: Option<DocumentInput>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InvalidateCacheRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureReviewRequest {
    pub request_id: Uuid,
    pub context_id: Uuid,
    pub options: ReviewOptions,
    pub documents: Vec<DocumentInput>,
    pub architecture_model: ArchitectureModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedArchitectureReviewRequest {
    pub request: ArchitectureReviewRequest,
}

impl CachedArchitectureReviewRequest {
    pub fn new(context_id: Uuid) -> Self {
        Self {
            request: ArchitectureReviewRequest {
                request_id: Uuid::new_v4(),
                context_id,
                options: ReviewOptions::create(),
                documents: Vec::new(),
                architecture_model: ArchitectureModel::new_with_context_id(context_id),
            },
        }
    }

    pub fn upsert_document(&mut self, document: DocumentInput) {
        if let Some(existing) = self
            .request
            .documents
            .iter_mut()
            .find(|existing| existing.filename == document.filename)
        {
            *existing = document;
        } else {
            self.request.documents.push(document);
        }
    }

    pub fn upsert_documents(&mut self, documents: Vec<DocumentInput>) {
        for document in documents {
            self.upsert_document(document);
        }
    }

    pub fn tracked_document(&self, file_path: &str) -> Option<&DocumentInput> {
        self.request
            .documents
            .iter()
            .find(|document| document.filename.as_deref() == Some(file_path))
    }

    pub fn request_for_review(
        &self,
        options: ReviewOptions,
        reviewed_document: Option<DocumentInput>,
    ) -> ArchitectureReviewRequest {
        let mut request = self.request.clone();

        request.request_id = Uuid::new_v4();
        request.options = options;
        request.documents = outbound_review_documents(request.documents);

        if let Some(reviewed_document) = reviewed_document {
            request
                .documents
                .push(sanitize_outbound_document(reviewed_document));
        }

        request
    }
}

fn outbound_review_documents(documents: Vec<DocumentInput>) -> Vec<DocumentInput> {
    documents
        .into_iter()
        .filter_map(|document| {
            if is_design_document(&document) {
                Some(document)
            } else if is_source_document(&document) {
                Some(strip_document_content(document))
            } else {
                Some(strip_code_content(document))
            }
        })
        .collect()
}

fn sanitize_outbound_document(document: DocumentInput) -> DocumentInput {
    if is_design_document(&document) {
        document
    } else if is_source_document(&document) {
        strip_document_content(document)
    } else {
        strip_code_content(document)
    }
}

fn strip_document_content(mut document: DocumentInput) -> DocumentInput {
    document.content.clear();
    document
}

fn strip_code_content(mut document: DocumentInput) -> DocumentInput {
    document.content = document
        .content
        .into_iter()
        .filter(|content| !matches!(content.content_type, ContentType::Code))
        .collect();

    document
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOptions {
    pub infer_stakeholders: bool,
    pub infer_architectural_decisions: bool,
    pub include_quality_attribute_ranking: bool,
    pub domains_to_review: Vec<Domain>,
    pub components_to_review: Vec<String>,
    pub minimum_confidence_threshold: f64,
    pub minimum_gap_severity: GapSeverity,
}

impl ReviewOptions {
    pub fn create() -> Self {
        Self {
            infer_stakeholders: true,
            infer_architectural_decisions: true,
            include_quality_attribute_ranking: true,
            domains_to_review: vec![
                Domain::Application,
                Domain::Integration,
                Domain::Data,
                Domain::Infrastructure,
                Domain::Security,
                Domain::Enterprise,
            ],
            components_to_review: vec![],
            minimum_confidence_threshold: 0.0,
            minimum_gap_severity: GapSeverity::Low,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GapSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInput {
    pub id: String,
    pub title: String,
    pub filename: Option<String>,
    pub type_hint: DocumentTypeHint,
    pub author: Option<String>,
    pub date: Option<String>,
    pub version: Option<String>,
    pub stated_scope: Option<String>,
    pub organization_context: Option<serde_json::Value>,
    pub known_stakeholders: Vec<serde_json::Value>,
    pub known_decisions: Vec<serde_json::Value>,
    pub content: Vec<DocumentContent>,
    pub data_hash: String,
    pub data_hash_algorithm: String,
    pub scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContent {
    pub content_type: ContentType,
    pub media_type: Option<String>,
    pub encoding: Option<ContentEncoding>,
    pub data: String,
    pub data_hash: String,
    pub data_hash_algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocumentTypeHint {
    ApplicationDesign,
    ArchitectureDecisionRecord,
    IntegrationDesign,
    DataModel,
    InfrastructureDesign,
    SecurityDesign,
    ThreatModel,
    EnterpriseRoadmap,
    StandardsDocument,
    Runbook,
    Codebase,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContentType {
    Text,
    Base64Pdf,
    Base64Img,
    Url,
    Code,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContentEncoding {
    Plain,
    Base64,
    Utf8,
}

pub type ArchModel = ArchitectureModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureModel {
    pub context_id: Option<Uuid>,
    pub context: ArchitectureContext,
    pub components: Vec<ArchitectureComponent>,
    pub relationships: Vec<ArchitectureRelationship>,
    pub global_observations: ArchitectureObservations,
    pub evidence: Vec<ArchitectureEvidence>,
    pub warnings: Vec<String>,
}

impl ArchitectureModel {
    pub fn new() -> Self {
        Self::new_with_context_id(Uuid::new_v4())
    }

    pub fn new_with_context_id(context_id: Uuid) -> Self {
        Self {
            context_id: Some(context_id),
            context: ArchitectureContext::new_with_context_id(context_id),
            components: Vec::new(),
            relationships: Vec::new(),
            global_observations: ArchitectureObservations::default(),
            evidence: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureComponent {
    pub component_id: String,
    pub name: String,
    pub component_type: ArchitectureComponentType,
    pub domain: Domain,
    pub root: Option<String>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub observations: ArchitectureObservations,
    pub evidence_refs: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Domain {
    Application,
    Integration,
    Data,
    Infrastructure,
    Security,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct ArchitectureRelationship {
    pub source_component_id: String,
    pub target_component_id: String,
    pub relationship_type: ArchitectureRelationshipType,
    pub protocol: Option<String>,
    pub evidence_refs: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
#[serde(rename_all = "camelCase")]
pub struct ArchitectureEvidence {
    pub evidence_id: String,
    pub source_type: String,
    pub path: Option<String>,
    pub description: String,
    pub scanned_at: Option<u64>,
}
