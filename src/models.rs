use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Domain {
    Application,
    Integration,
    Data,
    Infrastructure,
    Security,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ReviewMode {
    Single,
    Multiple,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ReviewPurpose {
    Full,
    Intermediate,
}

#[derive(Serialize)]
struct ReviewOptions {
    #[serde(rename = "inferStakeholders")]
    infer_stakeholders: bool,
    #[serde(rename = "inferArchitecturalDecisions")]
    infer_architectural_decisions: bool,
    #[serde(rename = "includeQualityAttributeRanking")]
    include_quality_attribute_ranking: bool,
    #[serde(rename = "domainsToReview")]
    domains_to_review: Vec<Domain>,
    #[serde(rename = "minimumConfidenceThreshold")]
    minimum_confidence_threshold: f64,
    #[serde(rename = "minimumGapSeverity")]
    minimum_gap_severity: GapSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GapSeverity {
    Low,
    Medium,
    High,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentInput {
    id: String,
    title: String,
    filename: String,
    type_hint: DocumentTypeHint,
    author: Option<String>,
    date: Option<String>,
    version: Option<String>,
    stated_scope: Option<String>,
    organization_context: Option<serde_json::Value>,
    known_stakeholders: Vec<serde_json::Value>,
    known_decisions: Vec<serde_json::Value>,
    content: Vec<DocumentContent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentContent {
    content_type: ContentType,
    media_type: String,
    encoding: ContentEncoding,
    data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DocumentTypeHint {
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
enum ContentType {
    Text,
    Base64Pdf,
    Base64Img,
    Url,
    Code,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ContentEncoding {
    Plain,
    Base64,
    Utf8,
}

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResponse {
    pub context_id: Uuid,
    pub context_percent_used: serde_json::Value,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct DomainEstimate {
    pub domain: Domain,
    pub present: bool,
    pub estimated_components: i32,
    pub complexity_modifier: ComplexityModifier,
    pub estimated_price: u64,
    pub rationale: String,
    pub confidence: f64,
    pub sufficient_for_high_fidelity_review: bool,
    pub supporting_evidence: Vec<String>,
    pub missing_context: Vec<String>,
    pub warnings: Vec<String>,
    pub review_targets: Vec<ReviewTargetEstimate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTargetEstimate {
    pub target_id: String,
    pub target_name: String,
    pub domain: Domain,
    pub target_type: String,
    pub complexity_modifier: ComplexityModifier,
    pub estimated_price: u64,
    pub confidence: f64,
    pub sufficient_for_high_fidelity_review: bool,
    pub supporting_evidence: Vec<String>,
    pub missing_context: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureReviewEstimates {
    #[serde(rename = "context_id")]
    pub context_id: String,
    pub status: String,
    pub question: String,
    #[serde(rename = "domain_estimates")]
    pub domain_estimates: Vec<DomainEstimate>,
    #[serde(rename = "sats_available")]
    pub sats_available: u64,
    #[serde(rename = "total_estimated_price")]
    pub total_estimated_price: u64,
    #[serde(rename = "requires_user_selection")]
    pub requires_user_selection: bool,
}

impl ArchitectureReviewEstimates {
    pub fn present_estimated_price(&self) -> u64 {
        self.domain_estimates
            .iter()
            .filter(|estimate| estimate.present)
            .map(|estimate| estimate.estimated_price)
            .sum()
    }

    pub fn selection_guidance(&self) -> Option<&'static str> {
        if self.requires_user_selection {
            Some("Present domain_estimates to the user. The selected domains' combined estimated_price must be less than or equal to sats_available. If the selection exceeds sats_available, ask the user to choose fewer domains or add more funds.")
        } else {
            None
        }
    }

    pub fn present_domains_exceed_available_balance(&self) -> bool {
        self.present_estimated_price() > self.sats_available
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthHeartbeat {
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthNResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: u64,
    pub status: Option<i32>,
    pub message: Option<String>,
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
struct AddContextRequest {
    #[schemars(with = "Option<String>")]
    context_id: Option<uuid::Uuid>,
    organization_context: Option<serde_json::Value>,
    business_goals: Option<Vec<String>>,
    stakeholders: Option<Vec<serde_json::Value>>,
    decisions: Option<Vec<serde_json::Value>>,
    constraints: Option<Vec<String>>,
    risks: Option<Vec<String>>,
    standards: Option<Vec<String>>,
    scope_notes: Option<Vec<String>>,
    freeform_notes: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScanProjectRequest {
    root_dir: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FullReviewEstimatesRequest {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FullReviewRequest {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IntermediateReviewRequest {
    file_path: String,
    content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EvaluateDocumentChangeRequest {
    file_path: String,
    content: String,
}

#[derive(Debug, Clone)]
struct DocumentChangeEvaluation {
    requires_intermediate_review: bool,
    reason: String,
    updated_document: Option<DocumentInput>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct InvalidateCacheRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureReviewRequest {
    pub request_id: Uuid,
    pub context_id: Uuid,
    pub review_mode: ReviewMode,
    pub review_purpose: ReviewPurpose,
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
                review_mode: ReviewMode::Multiple,
                review_purpose: ReviewPurpose::Full,
                options: ReviewOptions::full_review(),
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
        review_mode: ReviewMode,
        review_purpose: ReviewPurpose,
        options: ReviewOptions,
        reviewed_document: Option<DocumentInput>,
    ) -> ArchitectureReviewRequest {
        let mut request = self.request.clone();

        request.request_id = Uuid::new_v4();
        request.review_mode = review_mode;
        request.review_purpose = review_purpose;
        request.options = options;

        if let Some(reviewed_document) = reviewed_document {
            request.documents.push(reviewed_document);
        }

        request
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewMode {
    Single,
    Multiple,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewPurpose {
    Full,
    Intermediate,
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
    pub fn full_review() -> Self {
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

    pub fn intermediate_review() -> Self {
        Self {
            infer_stakeholders: false,
            infer_architectural_decisions: false,
            include_quality_attribute_ranking: false,
            domains_to_review: vec![
                Domain::Application,
                Domain::Integration,
                Domain::Data,
                Domain::Infrastructure,
                Domain::Security,
                Domain::Enterprise,
            ],
            components_to_review: vec![],
            minimum_confidence_threshold: 0.4,
            minimum_gap_severity: GapSeverity::High,
        }
    }
}

impl ReviewOptions {
    fn default_options() -> Self {
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

    fn intermediate_options() -> Self {
        Self {
            infer_stakeholders: false,
            infer_architectural_decisions: false,
            include_quality_attribute_ranking: false,
            domains_to_review: vec![
                Domain::Application,
                Domain::Integration,
                Domain::Data,
                Domain::Infrastructure,
                Domain::Security,
                Domain::Enterprise,
            ],
            minimum_confidence_threshold: 0.4,
            minimum_gap_severity: GapSeverity::High,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Domain {
    Application,
    Integration,
    Data,
    Infrastructure,
    Security,
    Enterprise,
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
pub struct ArchitectureModel {
    pub context_id: Option<Uuid>,
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
            components: Vec::new(),
            relationships: Vec::new(),
            global_observations: ArchitectureObservations::default(),
            evidence: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureComponent {
    pub component_id: String,
    pub name: String,
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