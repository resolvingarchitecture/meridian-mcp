# TODO

## MVP

### Application

- [x] Implement core MCP tools: `scan_project`, `review_file`, and `invalidate_cache`
- [x] Keep the binary thin; do not move AI logic, prompt construction, provider orchestration, billing logic, usage policy, review readiness, baseline ownership, or decision governance into MCP
- [x] Keep stdout reserved for MCP JSON-RPC and write logs/diagnostics to stderr
- [x] Reuse scanner, cache, and backend relay path for review flow
- [x] Replace remaining ArchGuard naming in code, packaging, configs, cache paths, logs, and user-facing messages with Meridian naming
- [x] Rename server/runtime identifiers from ArchGuard-oriented names to Meridian-oriented names
- [x] Align MCP server metadata, binary name, command name, and release artifacts with Meridian branding
- [x] Add CLI command dispatch while reusing the same scanner, cache, and backend client modules
- [x] Implement or wire recommended CLI commands:
  - [x] `meridian mcp`
  - [x] `meridian scan [root_dir]`
  - [x] `meridian review <file_path>`
  - [x] `meridian cache clear [root_dir]`
  - [x] `meridian config set api-key <key>`
  - [x] `meridian doctor`
  - [x] `meridian version`
- [x] Add support for session authentication upon successful backend auth with API key
- [ ] Improve error messages when backend auth fails, balance/usage policy blocks review, or the backend is unreachable
- [ ] Surface backend missing-context, targeted-question, and full-review recommendation responses clearly in MCP and CLI output
- [ ] Surface backend decision guidance, trade-offs, architectural stakeholder approval impact, affected parties, represented concerns, assumptions, and confidence clearly when present
- [ ] Ensure local review messaging says Meridian recommends and the customer organization decides
- [ ] Ensure local review messaging does not imply Resolving Architecture approves customer-specific architecture decisions
- [ ] Add end-to-end smoke tests for `scan_project`, `review_file`, and `invalidate_cache`
- [ ] Validate scan performance on large repositories
- [ ] Add fixture coverage for layered, DDD, clean architecture, hexagonal, and mixed-structure projects
- [ ] Make cache invalidation more explicit and observable
- [ ] Add local diagnostics for configuration, backend reachability, and cache health

### Skill/Service Workflow Support

- [ ] Document that backend Skill is synonymous with Resolving Architecture Service.
- [ ] Update MCP tool descriptions to use service/skill terminology consistently.
- [ ] Add response handling for skill workflow readiness states.
- [ ] Add response handling for `QUESTIONS_REQUIRED` or equivalent backend response.
- [ ] Add response handling for backend-generated skill workflow questions.
- [ ] Add local rendering for question prompts in CLI output.
- [ ] Add MCP response shape for questions so agentic clients can present them to users.
- [ ] Add compatibility handling for backend workflow IDs or correlation IDs.
- [ ] Add future MCP tool design for `start_skill_workflow`.
- [ ] Add future MCP tool design for `answer_skill_questions`.
- [ ] Ensure skill workflow support does not add local architecture judgment.
- [ ] Ensure skill workflow support does not add local rules execution.
- [ ] Ensure skill workflow support does not add local prompt construction.

### Local Architecture Model Ownership

- [ ] Document that MCP owns local Architecture Model persistence.
- [ ] Ensure Architecture Model cache remains local-only.
- [ ] Ensure Architecture Model is sent to backend only as transient request input.
- [ ] Add CLI/help copy explaining that backend does not persist raw Architecture Models.
- [ ] Add MCP tool description copy explaining that backend does not persist raw Architecture Models.
- [ ] Ensure user/agent answers to backend questions can enrich the local Architecture Model.
- [ ] Distinguish local Architecture Model context from backend analysis results in cache structures.
- [ ] Avoid storing backend findings or reports inside the Architecture Model unless explicitly required by a future contract.
- [ ] Add tests proving Architecture Model cache does not include backend-only product data.
- [ ] Add tests proving question answers are handled as local context unless explicitly submitted to backend-supported result/decision APIs.

### Backend Question Loop

- [ ] Parse backend-generated question arrays.
- [ ] Preserve question IDs for follow-up answer submission.
- [ ] Preserve answer type metadata.
- [ ] Preserve required/optional metadata.
- [ ] Preserve why-it-matters metadata.
- [ ] Preserve related skill/domain/stack metadata.
- [ ] Render missing-context categories clearly.
- [ ] Render proceed recommendations clearly.
- [ ] Add CLI smoke test for a questions-required response.
- [ ] Add MCP compatibility test for a questions-required response.
- [ ] Add malformed question response handling.

### Privacy Boundary

- [ ] Add tests ensuring raw source content is not written to logs during skill workflow requests.
- [ ] Add tests ensuring Architecture Model payloads are not logged.
- [ ] Add docs explaining that raw Architecture Models are not backend-persisted.
- [ ] Add docs explaining that backend persists only analysis results and privacy-safe metadata.
- [ ] Ensure local diagnostics do not print raw Architecture Model payloads by default.

### Scanner and Architecture Model

- [x] Implement repository traversal
- [x] Respect ignore rules during scanning
- [x] Infer likely architectural layers from path names
- [x] Parse imports for supported languages
- [x] Derive layer ordering from import relationships where possible
- [x] Detect common architectural patterns
- [x] Harvest ADR and architecture-document references
- [x] Include scan timestamp in the architecture model
- [ ] Define and document the exact `ArchModel` JSON contract used by the backend
- [ ] Add import graph summary to `ArchModel` if required by the backend contract
- [ ] Add dependency relationship summaries to `ArchModel` if required by the backend contract
- [ ] Represent ADR references and architecture-document references separately if the backend needs the distinction
- [ ] Include source root identity in the architecture model and review payload
- [ ] Include optional workspace identity in the architecture model and review payload when available
- [ ] Include optional backend architecture context ID in the review payload when available
- [ ] Add cache key or freshness metadata to the architecture model or review payload if needed
- [ ] Confirm what minimum metadata the backend needs for review fidelity
- [ ] Confirm what metadata the backend needs for source-to-baseline coverage and intermediate review eligibility
- [ ] Ensure file paths are normalized consistently before review requests are sent
- [ ] Ensure scan model remains compact and does not become a full source index
- [ ] Ensure scan model captures structural facts only and does not attempt to classify final architecture decisions locally

### Cache

- [x] Persist architecture model locally
- [x] Load cached model for review
- [x] Allow explicit cache invalidation
- [x] Derive cache key from project root and directory structure fingerprint
- [x] Avoid storing raw source content in the cache
- [x] Rename local cache directory from old product naming to Meridian naming
- [ ] Confirm cache location and cache key strategy are stable across supported platforms
- [ ] Track scan freshness metadata more explicitly
- [ ] Add cache health diagnostics for `meridian doctor`
- [ ] Validate cache file permissions on Linux, macOS, and Windows
- [ ] Ensure cache entries preserve source-root identity
- [ ] Ensure cache entries do not store customer-facing product data, architecture decision records, or recommendation outcomes

### Backend Integration

- [x] Use bearer authentication for backend requests
- [x] Send file review payloads to the backend instead of performing local AI review
- [x] Preserve backend error body for non-success responses
- [x] Replace old environment variable names with:
  - [x] `MERIDIAN_API_KEY`
  - [x] `MERIDIAN_BACKEND_URL`
  - [x] `MERIDIAN_LOG`
- [x] Confirm API key validation and messages align with `m_live_...` Meridian API keys
- [x] Replace generic `/api/review` usage with the backend's current review endpoint
- [x] Confirm review endpoint selection among:
  - [x] `POST /api/skills/review/full`
  - [x] `POST /api/skills/review/intermediate`
  - [x] `POST /api/skills/review/full/prompt`
- [ ] Confirm the review request payload matches the selected backend endpoint exactly
- [ ] Confirm the review response shape maps cleanly to MCP findings
- [ ] Confirm backend response mapping supports readiness states: `READY`, `PARTIAL`, and `INSUFFICIENT`
- [ ] Confirm backend response mapping supports full-review recommendation states:
  - [ ] `NO_FULL_REVIEW_NEEDED`
  - [ ] `FULL_REVIEW_RECOMMENDED`
  - [ ] `FULL_REVIEW_REQUIRED`
- [ ] Confirm backend response mapping supports missing baseline responses
- [ ] Confirm backend response mapping supports targeted follow-up questions
- [ ] Confirm backend response mapping supports provisional findings
- [ ] Confirm backend response mapping supports decision-related fields when present:
  - [ ] architectural decision vs supporting design classification
  - [ ] options considered
  - [ ] trade-offs
  - [ ] architectural stakeholders with approval authority
  - [ ] affected parties
  - [ ] represented concerns
  - [ ] assumptions
  - [ ] consequences
  - [ ] confidence
  - [ ] open questions
- [x] Surface authentication failures clearly
- [ ] Surface insufficient balance or usage-limit failures clearly
- [ ] Avoid crashing on malformed backend responses
- [ ] Add resilient retry behavior for transient backend failures where appropriate
- [ ] Verify usage, tier, pricing, payment, and balance enforcement remain backend-only
- [ ] Verify context sufficiency assessment, review readiness, full review baselines, intermediate review eligibility, and decision governance remain backend-only

### MCP

- [x] Expose `scan_project(root_dir)`
- [x] Expose `review_file(root_dir, file_path, content)`
- [x] Expose `invalidate_cache(root_dir)`
- [ ] Harden MCP tool input validation
- [ ] Confirm MCP tool descriptions match Meridian naming and current behavior
- [ ] Confirm MCP tool descriptions describe `meridian-mcp` as a local scanner/cache/relay, not the product-intelligence runtime
- [ ] Confirm MCP tool descriptions explain that intermediate reviews require a prior full review baseline
- [ ] Confirm MCP tool descriptions explain that missing baseline should lead to a full review recommendation
- [ ] Confirm MCP tool descriptions explain that Meridian recommends and the customer organization decides
- [x] Ensure MCP mode startup does not print non-protocol output to stdout
- [ ] Document local configuration for Cursor, Claude Code, and VS Code MCP usage
- [ ] Add compatibility tests for MCP request/response behavior
- [ ] Add compatibility tests for readiness, missing baseline, missing context, full-review recommendation, and decision-guidance response categories

### CLI

- [x] Bundle CLI workflows in the same local client component as MCP
- [x] Reuse scanner, cache, and backend relay path for CLI commands
- [ ] Ensure `meridian review <file_path>` renders backend readiness, missing-context, and missing-baseline responses clearly
- [ ] Ensure `meridian review <file_path>` renders full-review recommendation states clearly
- [ ] Ensure `meridian review <file_path>` renders decision guidance as advisory, not as an approved customer decision
- [ ] Ensure `meridian doctor` validates local configuration, backend reachability, and cache health
- [ ] Ensure CLI errors do not leak raw source content, API keys, or secrets
- [ ] Add CLI smoke tests for scan, review, cache clear, config, doctor, and version commands

### Infrastructure and Release

- [ ] Ensure release workflow emits Meridian-named artifacts
- [ ] Verify Linux x86_64 release target builds
- [ ] Verify macOS x86_64 release target builds
- [ ] Verify macOS arm64 release target builds
- [ ] Verify Windows x86_64 release target builds
- [ ] Update install script references to Meridian endpoints, names, binary paths, and commands
- [ ] Remove old ArchGuard references from release and packaging automation
- [ ] Publish a clearer release matrix in README and release workflow
- [ ] Confirm `cargo install`, install script, and GitHub Releases distribution paths all use Meridian naming
- [ ] Confirm release notes describe the binary as the single local client install containing MCP server and CLI

### Documentation

- [x] Update design direction around Meridian/backend-owned product intelligence
- [x] Document current intended module boundaries
- [x] Document current backend API surface
- [x] Document MCP tool responsibilities
- [x] Document cache design and non-goals
- [x] Document Rule Miner boundary as out of scope for MCP
- [x] Document bundled CLI as part of the same local client component
- [ ] Update README to match Meridian naming and current `DESIGN.md`
- [ ] Update setup instructions for `MERIDIAN_API_KEY`, `MERIDIAN_BACKEND_URL`, and `MERIDIAN_LOG`
- [ ] Document CLI commands once implemented
- [ ] Document migration path from older ArchGuard naming
- [ ] Document backend endpoint compatibility expectations
- [ ] Document review response categories:
  - [ ] findings
  - [ ] missing context
  - [ ] missing baseline
  - [ ] full review recommended
  - [ ] full review required
  - [ ] decision guidance
  - [ ] error
- [ ] Document the local/backend boundary for Agentic Architect workflows
- [ ] Document that customer-owned decision authority remains outside MCP

### Security and Privacy

- [x] Avoid direct AI provider SDK usage in the local binary
- [x] Avoid local billing, pricing, payment, usage, account, or product-data persistence
- [x] Avoid storing raw source code in the architecture cache
- [ ] Audit logging to ensure file contents, API keys, and secrets are never logged
- [ ] Confirm backend auth failures are surfaced without leaking sensitive details
- [ ] Review review-payload construction to ensure only necessary file content and model metadata are sent
- [ ] Consider optional local redaction controls before review requests are sent
- [ ] Confirm local config storage does not expose API keys unnecessarily
- [ ] Confirm local cache does not store raw customer source, decision records, stakeholder records, or recommendation outcomes
- [ ] Confirm local telemetry, if added later, remains privacy-safe and does not store raw customer source

## Backlog

### Application

- [ ] Improve architecture pattern detection heuristics
- [ ] Expand fixture coverage for language edge cases and malformed syntax
- [ ] Add richer local diagnostics without adding product judgment locally
- [ ] Add startup and scan timing metrics for local diagnostics
- [ ] Improve terminal output formatting for CLI mode
- [ ] Add clearer workspace/source-root association diagnostics when backend architecture contexts are supported
- [ ] Add local display support for backend decision prompts without storing decision records locally

### Scanner and Architecture Model

- [ ] Add richer scan metadata if the backend starts using it
- [ ] Improve import parsing for Rust and Go if backend review benefits from it
- [ ] Improve topological ordering behavior for cycles and ambiguous dependencies
- [ ] Record lightweight scan diagnostics for debugging stale model issues
- [ ] Add architecture-document metadata beyond first-heading summaries if needed
- [ ] Add structured metadata for ADR collections, API contracts, infrastructure roots, data model files, security artifacts, and operations artifacts if the backend source contract requires it

### Multi-root and Source Context

- [ ] Consider future `scan_roots(root_dirs, context_id?)` MCP tool if backend and IDE workflows require first-class multi-root support
- [ ] Consider future `get_project_status(root_dir?, root_dirs?, context_id?)` MCP tool if IDEs need explicit readiness/status discovery
- [ ] Preserve root identity for multi-root payloads
- [ ] Avoid locally merging multiple roots into one architecture model unless the backend contract explicitly requires it
- [ ] Support backend source IDs or source-role metadata if the backend introduces structured architecture sources
- [ ] Treat local filesystem paths as hints, not durable global source identity

### Integration

- [ ] Add support for future backend review fields without breaking compatibility
- [ ] Add explicit compatibility tests against backend API revisions
- [ ] Consider a migration helper for users upgrading from ArchGuard naming
- [ ] Add a backend capability/version check if the API introduces one
- [ ] Add compatibility handling for future architecture source contracts
- [ ] Add compatibility handling for future architecture decision record contracts while keeping decision persistence backend-owned

### Infrastructure

- [ ] Add benchmark automation for scan and startup latency
- [ ] Add CI checks for MCP protocol-safe stdout behavior
- [ ] Add CI checks to prevent reintroducing old product naming
- [ ] Add release smoke tests for generated binaries

### Security

- [ ] Validate cache file permissions on all supported platforms
- [ ] Add tests proving logs do not include file content or API keys
- [ ] Consider config-file encryption or OS keychain support for stored API keys
- [ ] Add tests proving decision guidance and backend response rendering do not persist product data locally