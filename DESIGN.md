# Meridian MCP — Design

## Purpose

`meridian-mcp` is the local, open-source MCP server and bundled CLI runtime that runs on a developer's machine.

It is the single local Meridian client install.

Its job is to:

1. Scan one or more local sources and build or extend a cached lightweight architecture model
2. Add discovered supporting context, such as ADRs, architecture documents, contracts, deployment files, or other non-code sources, to that same model when available
3. Reuse the cached architecture model for full-review prompt generation, full review, and intermediate review workflows
4. Expose MCP tools to IDEs and MCP-compatible clients
5. Provide CLI commands for setup, diagnostics, scanning, cache control, and manual review
6. Review saved files or manually selected files by relaying requests to the Meridian backend
7. Surface backend review findings, readiness responses, missing context, baseline state, and full-review recommendations clearly

It must stay thin. All architectural judgment, context sufficiency assessment, review readiness 
decisions, prompt construction, provider orchestration, billing enforcement, full review baseline 
management, decision governance, rule lifecycle management, and persistence of product data belong 
in the backend or internal services.

Meridian's product position is broader than file review. Meridian is an **Agentic Architect**: an
architecture assistant for assessment, design support, feasibility analysis, technology evaluation, 
transition planning, and governance.

`meridian-mcp` supports that product model locally by collecting structural project facts and 
relaying requests. It does not decide architecture, approve architecture decisions, own stakeholder 
consensus, or act as the customer's accountable technical authority.

Short form:

> Meridian assists. Customer leaders decide. Resolving Architecture improves Meridian.

---

## Design goals

- **Single local client install**: ship MCP server, CLI, scanner, cache, and backend relay in one binary
- **Local-first execution**: run on the developer machine as a standalone binary
- **Fast startup and scan**: produce a usable architecture model quickly
- **Minimal dependencies**: no Node runtime, no JVM, no external services required locally
- **Privacy-preserving**: only file content and model metadata needed for the requested workflow are sent to the backend
- **MCP-compatible**: speak MCP over stdio and behave well with Cursor, Claude Code, VS Code integrations, and other MCP hosts
- **CLI-compatible**: provide terminal workflows using the same scanner, cache, and backend relay path as MCP tools
- **Backend-driven intelligence**: no AI calls from the binary itself
- **Baseline-aware intermediate review**: only relay intermediate review requests when the backend can evaluate them against a prior full review baseline
- **Decision-aware transport**: preserve enough context for the backend to classify recommendations, trade-offs, required architectural stakeholders, affected parties, represented concerns, assumptions, and decision implications where supported
- **Stable caching**: avoid rescanning unchanged projects unnecessarily

## Multi-root and multi-source architecture context

Meridian must not assume that a meaningful architecture review maps to exactly one local project root.

Real systems commonly span multiple roots and information sources, including:

- frontend applications;
- backend services;
- mobile applications;
- shared libraries;
- infrastructure repositories;
- deployment manifests;
- API contract repositories;
- data model or analytics repositories;
- ADR and architecture-document repositories;
- security, compliance, or operational documentation;
- vendor or integration reference material.

Each source may be scanned separately, but several sources may also contribute to one larger cached architecture model 
for the same Meridian architecture context or full review baseline.

Examples:

| Review shape                  | Meaning                                                                          |
|-------------------------------|----------------------------------------------------------------------------------|
| Single-root review            | One application or repository is reviewed on its own                             |
| Multi-root application review | Frontend, backend, infrastructure, and ADR roots are reviewed as one application |
| Portfolio or platform review  | Multiple applications or services are reviewed as part of a larger system        |
| Source-of-information review  | Code, docs, diagrams, ADRs, deployment files, and API contracts are evidence     |

The MCP client should therefore treat `root_dir` as a local source root, not as the whole architecture by default.

A Meridian architecture context may be associated with one root, several roots, or non-code sources. 
The backend remains responsible for deciding how those sources relate to a full review baseline, review scope, 
and intermediate review eligibility.

The local client should support this model by:

- scanning one or more sources independently
- building and appending to one lightweight cached architecture model for the selected Meridian architecture context where supported
- adding discovered non-code evidence as context to the model instead of treating it as a separate review target by default
- preserving source identity in the model and review requests
- distinguishing the local workspace root from the backend architecture context
- avoiding assumptions that one IDE workspace equals one reviewed system
- allowing future IDEs and agents to associate multiple local sources with a single Meridian context

---

## Current product role

The current Meridian architecture places product intelligence in the backend and internal services, not in the MCP binary.

`meridian-mcp` is responsible for local execution and local architecture facts:

- walk the repository
- infer project structure
- parse imports
- detect architectural patterns
- harvest ADR references
- cache the result
- expose MCP tools
- expose bundled CLI commands
- send review payloads to the backend
- render or relay backend responses

The backend owns:

- access validation
- usage and payment enforcement
- context sufficiency assessment
- review readiness decisions
- full review baseline creation and lookup
- intermediate review eligibility
- deterministic rules-assisted review
- prompt construction
- AI provider orchestration
- finding and report validation
- recommendation quality controls
- architecture decision records
- trade-off and stakeholder-consensus workflows
- product telemetry and privacy-safe learning

The backend currently exposes:

- `GET /api/pricing/token`
- `GET /api/payment/request/bitcoin`
- `GET /api/payment/request/fiat`
- `GET /api/payment/request/bitcoin/status/{address}`
- `GET /api/payment/request/fiat/status/{id}`
- `POST /api/skills/context`
- `POST /api/skills/review/full/prompt`
- `POST /api/skills/review/full`
- `POST /api/skills/review/intermediate`
- `GET /api/usage/list`

The review endpoints are expected to support a backend-owned workflow where:

- full reviews perform context sufficiency assessment before final report generation;
- insufficient full-review context returns missing context, limitations, and targeted questions;
- completed full reviews create the baseline for future intermediate reviews;
- intermediate reviews are evaluated against a prior full review baseline;
- intermediate reviews recommend another full review when the change materially affects assumptions, scope, or risk posture;
- architectural recommendations include enough structure for options, trade-offs, architectural stakeholders with approval authority, affected parties, represented concerns, assumptions, consequences, and confidence when supported by the backend;
- accepted architectural decisions remain customer-owned and require an accountable decision maker, rationale, decision date, and consensus state.

The binary authenticates with a bearer API key using the `m_live_...` format.

---

## High-level flow

```text
IDE / MCP client 
    → meridian-mcp MCP server mode 
    → scan one or more local sources 
    → build or extend cached ArchModel 
    → add discovered ADRs, architecture docs, contracts, deployment files, and other context where available
    → on full-review prompt or full review, send cached ArchModel to backend
    → on file/intermediate review, send file content 
        + cached ArchModel to backend 
            → backend validates API key 
            → backend enforces usage and payment policy 
            → backend locates prior full review baseline for intermediate review 
            → backend runs rules-assisted review when eligible 
            → backend builds prompt 
            → backend calls AI provider 
            → backend validates and normalizes findings 
            → backend returns findings, missing context, decision guidance, or full-review recommendation 
            ← meridian-mcp forwards response to IDE
Terminal / developer 
    → meridian-mcp CLI command 
    → use same scanner, cache, and backend relay path 
    ← terminal output
```

`meridian-mcp` is therefore a local scanner/cache/transport layer, not the product-intelligence runtime.

---

## Responsibilities

### `meridian-mcp`

- MCP server lifecycle
- CLI command lifecycle
- MCP tool registration
- terminal command handling
- filesystem scanning
- architecture model inference
- import graph summarization
- ADR and architecture-document discovery
- local cache read/write
- backend HTTP client
- bearer authentication header handling
- response passthrough and error shaping
- local diagnostics for configuration, backend reachability, and cache health
- preservation of source-root identity in review requests
- rendering backend-owned review, readiness, missing-context, baseline, and full-review recommendation states

### Not `meridian-mcp`

- AI provider selection
- direct AI provider calls
- deterministic rules execution for final product judgment
- context sufficiency assessment
- full review readiness decisions
- full review baseline creation or ownership
- intermediate review eligibility decisions
- deciding whether another full review is needed
- prompt assembly
- billing enforcement
- pricing calculations
- usage tracking
- payment handling
- API key issuance
- session issuance
- user/account persistence
- customer-facing product data persistence
- architecture decision authority
- stakeholder consensus management
- trade-off validation
- recommendation outcome learning
- rule mining
- candidate-rule promotion
- admin governance workflows

---

## Internal modules

The current source layout is expected to remain centered around the local runtime:

- `main.rs`
- `agent.rs`
- `cache.rs`
- `scanner/mod.rs`
- `scanner/walker.rs`
- `scanner/imports.rs`
- `scanner/patterns.rs`
- `scanner/adrs.rs`

Future CLI-specific code should remain in the local client boundary and reuse scanner, cache, and backend client modules rather than duplicating them.

### `main.rs`

Entry point and local runtime bootstrap.

Responsibilities:

- initialize logging
- load environment and local configuration
- select MCP server mode or CLI command mode
- register MCP tools
- dispatch MCP requests to scanner / cache / agent modules
- dispatch CLI commands to the same local scanner / cache / agent path
- keep stdout protocol-safe in MCP mode

### `scanner/walker.rs`

Repository traversal and path collection.

Responsibilities:

- walk directories
- respect ignore rules
- identify candidate source and documentation files
- infer likely application layers from paths and names
- skip irrelevant files aggressively

### `scanner/imports.rs`

Dependency graph parsing and ordering.

Responsibilities:

- parse imports using available language support
- construct import graph summaries
- derive dependency relationships relevant to architecture review
- compute topological ordering where useful

### `scanner/patterns.rs`

Pattern detection.

Responsibilities:

- detect repeated architectural signatures
- classify common project structure patterns
- contribute to the architecture model

### `scanner/adrs.rs`

Architecture document harvesting.

Responsibilities:

- detect ADR and architecture-related documents
- extract lightweight metadata
- include references in the scanned model

### `cache.rs`

Local cache storage.

Responsibilities:

- persist `ArchModel`
- load cached models on review
- invalidate stale entries
- derive cache keys from project root + structure fingerprint
- keep cache safe to discard and rebuild

### `agent.rs`

Backend client.

Responsibilities:

- construct HTTP requests to backend
- include bearer auth header
- send file review payloads
- decode findings from backend responses
- preserve useful backend error details
- surface backend responses for insufficient context, missing baseline, and full-review recommendation states
- surface backend responses that include decision implications, trade-offs, affected stakeholders, or follow-up questions
- return errors in a form MCP clients and CLI workflows can surface cleanly

### MCP capability and tool discovery

`meridian-mcp` must communicate its capabilities and available tools to the IDE, MCP host, or agent through the MCP protocol.

Registration of the MCP server only makes Meridian available to the host. The host still needs to discover what Meridian provides before it can decide when and how to invoke Meridian tools.

During MCP initialization and tool discovery, `meridian-mcp` should provide:

| Metadata                   | Purpose                                                                                                      |
|----------------------------|--------------------------------------------------------------------------------------------------------------|
| Server identity            | Identify the server as Meridian and expose version/build information where practical                         |
| Protocol compatibility     | Confirm the MCP protocol version and supported capability set                                                |
| Tool names                 | List callable Meridian tools                                                                                 |
| Tool descriptions          | Explain what each tool does and when it should be used                                                       |
| Input schemas              | Define required and optional arguments for each tool                                                         |
| Usage constraints          | Explain baseline, context, authentication, and cache requirements                                            |
| Expected result categories | Help hosts distinguish findings, missing context, missing baseline, and full-review recommendation responses |
| Error semantics            | Help IDEs and agents display actionable messages                                                             |

Tool metadata is part of the product contract. It should be clear enough for both deterministic IDE extensions and agentic IDE hosts.

A deterministic IDE extension may call tools from fixed lifecycle events such as workspace open, manual command execution, or file save. An agentic IDE host may decide whether to call Meridian based on the tool names, descriptions, and schemas. For that reason, descriptions must be accurate and must not imply that intermediate review is a standalone first-contact review.

Tool descriptions should make Meridian's review model explicit:

- full reviews establish the durable architecture baseline;
- intermediate reviews evaluate changes against a prior full review baseline;
- if no prior baseline exists, the backend may recommend starting a full review;
- if an intermediate review discovers significant change, the backend may recommend or require another full review;
- recommendations may include decision implications, trade-offs, assumptions, affected stakeholders, and confidence when the backend supports those fields;
- the customer organization retains final decision authority;
- `meridian-mcp` relays and surfaces those responses, but does not make readiness, baseline, decision-authority, or full-review recommendation decisions locally.

The MCP server should keep tool metadata aligned with backend API contracts. If backend review request shapes, readiness states, baseline requirements, decision fields, or full-review recommendation states change, the corresponding MCP tool definitions and descriptions must be updated.

---

## MCP tools

The binary exposes Meridian tools through MCP tool discovery. Each tool should include a stable name, clear description, input schema, usage constraints, and response expectations.

The core MCP tool set is:

| Tool                         | Responsibility                                                                  |
|------------------------------|---------------------------------------------------------------------------------|
| `scan_project(root_dir)`     | Build or extend the cached local architecture model from one local source root   |
| `review_prompt(...)`         | Ask the backend for full-review readiness or prompt guidance using cached model  |
| `review_full(...)`           | Request a full architecture review using cached model context                    |
| `review_file(...)`           | Review one file or change against cached local facts and backend baseline policy |
| `invalidate_cache(root_dir)` | Drop cached model data and force a fresh scan                                    |

Additional tools such as `scan_sources`, `scan_roots`, or `get_project_status` may be added when the backend and IDE workflow need first-class multi-source or status discovery support.

The tool list is consumed by both:

- deterministic IDE integrations that call known tools from fixed lifecycle events; and
- agentic MCP hosts that choose tools based on descriptions and schemas.

Tool descriptions must be written carefully because they influence when agents decide to invoke Meridian.

Tools that accept `root_dir` operate on a local source root. They must not imply that one root is necessarily the entire architecture under review. A backend architecture context may include multiple roots and non-code sources.

Where practical, future tool versions should support either:

- a single `root_dir` for source-scoped operations;
- a `root_dirs` array for workspace, application, or context-scoped operations involving several roots; or
- a source list that can include code roots, document roots, contracts, infrastructure files, diagrams, or other architecture evidence.

### `scan_project(root_dir)`

Scans one local source root and builds or extends the cached architecture model for the relevant local architecture context.

Use when:

- a project or source root is opened;
- the user asks for a refresh;
- the cache is missing or stale;
- the user switches branches or performs a significant refactor;
- architecture-relevant files, ADRs, contracts, deployment files, or architecture documents change.

Input schema:

| Field      | Required | Purpose                                                         |
|------------|----------|-----------------------------------------------------------------|
| `root_dir` | Yes      | Absolute or host-resolved path to the source root being scanned |

Expected response:

- scan completed successfully;
- scan failed with actionable diagnostics;
- cached model created, extended, updated, or reused, depending on implementation.

This tool performs local source analysis only. It should not make final architecture judgments.

### `review_prompt(context_id?)`

Requests backend-owned full-review prompt guidance using the cached architecture model.

Use when:

- a user or IDE agent wants to start a full architecture review;
- the local architecture model has already been built from one or more scanned sources;
- the backend needs to assess whether submitted and stored context is sufficient before full-review execution;
- the backend may need to return missing context, limitations, targeted questions, or domain-selection guidance.

Input schema:

| Field        | Required | Purpose                                                                    |
|--------------|----------|----------------------------------------------------------------------------|
| `context_id` | No       | Meridian architecture context associated with the relevant backend context |

Expected response categories:

| Category             | Meaning                                                                           |
|----------------------|-----------------------------------------------------------------------------------|
| Ready                | Backend has enough context to proceed                                             |
| Partial              | Backend can proceed only with clearly stated limitations or provisional guidance   |
| Insufficient context | More context is needed before a useful full review can be produced                |
| Missing context      | Backend returned specific missing information or targeted questions               |
| Domain guidance      | Backend returned domain-selection or pricing-related prompt information           |
| Error                | Authentication, usage, backend, parsing, or transport failure                     |

This tool should:

- load the cached architecture model;
- include source identities represented in the cached model;
- include context identity when available;
- forward the request to the backend full-review prompt endpoint;
- let the backend determine context sufficiency, readiness, and prompt requirements;
- return backend guidance clearly to the client.

This tool should not require a file name or file content. It should not construct final prompts locally.

### `review_full(context_id?)`

Requests a full architecture review using the cached architecture model and backend-owned full-review policy.

Use when:

- the user explicitly requests a full review;
- the backend prompt/readiness stage indicates the context is ready or acceptable to proceed;
- one or more local sources have already contributed to the cached architecture model;
- the backend will create or update the full-review baseline if the review completes successfully.

Input schema:

| Field        | Required | Purpose                                                                    |
|--------------|----------|----------------------------------------------------------------------------|
| `context_id` | No       | Meridian architecture context associated with the relevant backend context |

Expected response categories:

| Category             | Meaning                                                                                       |
|----------------------|-----------------------------------------------------------------------------------------------|
| Full review report   | Backend produced a full architecture review                                                    |
| Missing context      | Backend needs more context before producing a credible full review                             |
| Partial              | Backend returned limited or provisional output with stated limitations                         |
| Decision guidance    | Backend returned options, trade-offs, approval stakeholders, affected parties, or assumptions |
| Baseline created     | Backend created or updated a full-review baseline where supported                              |
| Error                | Authentication, usage, backend, parsing, or transport failure                                  |

This tool should:

- load the cached architecture model;
- include all source identities represented in the cached model;
- include context identity when available;
- forward the request to the backend full-review endpoint;
- let the backend determine readiness, baseline creation, and review scope;
- surface backend missing-context, readiness, decision-guidance, and baseline responses clearly.

This tool should not require a file name or file content. It should not create a review baseline locally and should not make review-readiness decisions locally.

### `review_file(root_dir, file_path, content, context_id?)`

Reviews one file or change using the cached architecture model and backend-owned review policy.

Use when:

- a file is saved and automatic intermediate review is enabled;
- a user explicitly requests review of the current file;
- an IDE agent is asked whether a change conflicts with the established architecture;
- the project has a Meridian architecture context and prior full review baseline;
- the backend can determine whether a full review, intermediate review, or missing-context response is appropriate.

Input schema:

| Field        | Required | Purpose                                                                    |
|--------------|----------|----------------------------------------------------------------------------|
| `root_dir`   | Yes      | Absolute or host-resolved path to the source root containing the file      |
| `file_path`  | Yes      | Path of the file being reviewed                                            |
| `content`    | Yes      | Current file content to evaluate                                           |
| `context_id` | No       | Meridian architecture context associated with the relevant backend context |

Expected response categories:

| Category                 | Meaning                                                                                       |
|--------------------------|-----------------------------------------------------------------------------------------------|
| Findings                 | Review findings were produced                                                                 |
| Missing baseline         | No prior full review baseline exists for the relevant context/scope                           |
| Full review recommended  | The change may affect prior review assumptions or risk posture                                |
| Full review required     | The change materially exceeds or invalidates the prior baseline                               |
| Insufficient context     | More context is needed before the backend can provide useful output                           |
| Decision guidance        | Backend returned options, trade-offs, approval stakeholders, affected parties, or assumptions |
| Error                    | Authentication, usage, backend, parsing, or transport failure                                 |

This tool should:

- load the cached model for the relevant architecture context or source root;
- include source root identity in the backend request;
- include context identity when available;
- if necessary, trigger or request a fresh scan for that root;
- forward the review request to the backend;
- let the backend determine whether the associated context has a prior full review baseline;
- let the backend determine how the source root relates to the larger reviewed system;
- return findings to the client when review is eligible;
- return clear missing-context and targeted-question responses when context is insufficient;
- return a clear full-review recommendation when no baseline exists;
- return a clear full-review recommended or required message when the submitted change materially affects the prior baseline;
- relay any backend-supplied decision guidance without treating it as a locally approved architecture decision.

This tool should not create a review baseline locally and should not be described as a standalone full architecture review.

### Future `scan_sources(sources, context_id?)`

A future MCP tool may scan several local sources and build or extend one cached architecture model for the selected Meridian architecture context.

Use when:

- an IDE workspace contains multiple applications or repositories;
- a user or agent wants to prepare several roots for the same Meridian architecture context;
- frontend, backend, infrastructure, and documentation roots should be reviewed together;
- an architecture context spans multiple local folders or document sources;
- code, ADRs, contracts, deployment manifests, and supporting documentation should all contribute evidence to one model.

This tool should preserve source identity while appending discovered architecture facts and context into the cached model. The backend remains responsible for interpreting whether those sources belong to one review scope, several independent scopes, or a larger portfolio context.

---

## Bundled CLI commands

The same binary should also support CLI workflows.

Recommended commands:

| Command                             | Responsibility                                                       |
|-------------------------------------|----------------------------------------------------------------------|
| `meridian mcp`                      | Start MCP stdio server                                               |
| `meridian scan [root_dir]`          | Scan project and cache architecture model                            |
| `meridian review <file_path>`       | Review one file from the terminal                                    |
| `meridian cache clear [root_dir]`   | Clear local cache                                                    |
| `meridian config set api-key <key>` | Store local API key                                                  |
| `meridian doctor`                   | Validate local configuration, backend reachability, and cache health |
| `meridian version`                  | Print version and build metadata                                     |

CLI commands must reuse the same scanner, cache, and backend relay path used by MCP tools.

The CLI should not introduce separate architecture judgment, separate backend contracts, separate product policy, or separate decision governance behavior.

---

## Architecture model

The architecture model is intentionally compact.

It should capture:

- model identity;
- source identities for all scanned roots or inputs represented in the model;
- project root paths where applicable;
- optional workspace identity;
- optional architecture context ID;
- source type or role where known, such as codebase, ADR collection, API contract, infrastructure, data model, security document, operations document, or vendor reference;
- inferred layers;
- layer order;
- inferred architectural style;
- repeated patterns;
- import graph summary;
- dependency relationships relevant to architecture review;
- ADR references;
- architecture-document references;
- other discovered supporting context references;
- scan timestamp;
- cache key or freshness metadata.

It should not try to be a complete code index.

A single scanned source may create a local architecture model. Later scans may append additional structural facts or supporting context to that same model when they are associated with the same Meridian architecture context. A larger Meridian review may combine code-derived facts with user-provided architecture context and other sources of information.

The MCP client should not decide that multiple sources form one system of record. It should preserve source identity and provide scanned facts to the backend. The backend owns the interpretation of whether sources should be reviewed independently or together as part of a larger full review baseline.

### Design principle

The model should be good enough to ground backend review, not so large that it becomes expensive to compute, cache, transmit, or reason over.

The model should prefer structural facts over full source retention.

The model should help the backend identify architecture evidence, but it should not attempt to classify final recommendations, stakeholder consensus, or customer decision authority locally.

---

## Cache design

### Cache key

The current design uses a key derived from:

- source root path;
- directory structure fingerprint;
- optional workspace identity;
- optional architecture context ID when a root is explicitly associated with a Meridian context.

A cache entry represents the scanned architecture model for a specific source root. Multiple cache entries may later be associated with the same Meridian architecture context or full review baseline.

### Cache behavior

- cache is local only;
- cache lives under the user's home cache directory;
- cache should be fast to read and write;
- cache should be safe to discard and rebuild;
- cache should preserve source root identity;
- cache should support multiple roots per workspace;
- cache should support roots that belong to the same backend architecture context;
- cache should not persist user account data;
- cache should not persist decision records;
- cache should not become a long-term product data store.

### Invalidation

Invalidate when:

- the directory structure changes;
- the user explicitly invalidates a root;
- the user explicitly invalidates all roots in a workspace or context;
- the model is missing;
- the cache appears stale;
- a major refactor changes architecture-relevant structure;
- a root is detached from or re-associated with a different architecture context.

### Non-goals

- content-addressable indexing of every source file
- persistence across machines
- long-term historical model storage
- customer telemetry storage
- raw source-code retention for learning
- architecture decision record storage
- stakeholder consensus state storage

---

## Backend integration

The backend is the source of truth for:

- authenticated access
- review policy
- context sufficiency assessment
- full review readiness decisions
- full review baseline creation and lookup
- intermediate review eligibility
- recommendations for when another full review is needed
- usage enforcement
- payment and balance policy
- pricing policy
- deterministic rules-assisted review
- prompt construction
- provider selection
- finding normalization
- finding/report validation
- architecture decision records
- trade-off validation
- stakeholder and consensus workflows
- recommendation outcome feedback
- product data persistence

### Current backend package boundary

`meridian-backend` is a Java / Spring Boot modular monolith with feature-oriented packages.

The MCP binary should treat the backend as an HTTP API and should not duplicate backend package logic.

Relevant backend responsibilities include:

| Backend area  | Responsibility                                                                                  |
|---------------|-------------------------------------------------------------------------------------------------|
| `api`         | API-key issuance, validation, request filtering, authenticated context                          |
| `security`    | Session issuance, validation, invalidation, and recovery flows                                  |
| `user`        | User account state, balances, API-key/session links                                             |
| `payment`     | Payment orchestration, provider selection, reconciliation, and crediting                        |
| `usage`       | Append-only usage ledger and spend records                                                      |
| `pricingrule` | Deterministic pricing calculations                                                              |
| `context`     | User-provided architecture context storage, validation, billing, usage, and sufficiency support |
| `decision`    | Architecture decision records, trade-offs, consensus state, and customer decision authority     |
| `agent`       | AI provider abstraction and model invocation                                                    |
| `rules`       | Deterministic architecture rules and validation                                                 |
| `skills`      | Skill lifecycle, review readiness, full reviews, baselines, and intermediate flows              |
| `admin`       | Private administrative workflows and visibility                                                 |
| `infra`       | Shared database, AWS, and technical infrastructure                                              |

### Current backend API surface

The local client should stay aligned with the backend's actual published endpoints.

Current backend-facing product APIs include:

- `GET /api/pricing/token`
- `GET /api/payment/request/bitcoin`
- `GET /api/payment/request/fiat`
- `GET /api/payment/request/bitcoin/status/{address}`
- `GET /api/payment/request/fiat/status/{id}`
- `POST /api/skills/context`
- `POST /api/skills/review/full/prompt`
- `POST /api/skills/review/full`
- `POST /api/skills/review/intermediate`
- `GET /api/usage/list`

Review-related MCP calls should map to the review endpoints the backend currently exposes, rather than assuming a separate generic `/api/review` endpoint unless the backend introduces one.

### Review request shape

A review request conceptually includes:

- cached architecture model for the relevant Meridian architecture context;
- source identities represented in the model;
- optional workspace identity;
- optional review scope;
- optional user-provided architecture context;
- optional context id or baseline lookup key;
- optional source identity or local root hint;
- project root or equivalent project identity fields as required by the backend contract;
- file path and file content only for file-specific or intermediate review workflows.

Full reviews and intermediate reviews have different backend-owned requirements:

| Review type         | Backend-owned requirement                                               |
|---------------------|-------------------------------------------------------------------------|
| Full review prompt  | Enough cached or submitted context to assess readiness and guide review |
| Full review         | Enough submitted or stored context to create a credible review baseline |
| Intermediate review | Prior full review baseline for the same relevant architecture scope     |

A backend architecture context may include one source root, several source roots, or non-code sources. The local client may provide architecture models and file content when needed, but it must not decide whether roots form one review scope, whether a review is ready, whether a baseline exists, whether a recommendation is an accepted decision, or whether another full review is required.

Full-review prompt and full-review requests should use the cached architecture model directly. They should not require a file name or file content.

### Authentication

Requests use:

- `Authorization: Bearer <Meridian API key>`

The local client may also support local configuration helpers for storing or validating the API key, but API-key issuance and account state remain backend-owned.

### Error handling

The binary should:

- surface authentication failures clearly
- surface insufficient balance or usage-limit failures clearly
- surface insufficient full-review context responses clearly
- surface missing prior full review baseline responses clearly
- surface full-review recommended or required responses clearly
- surface decision-related missing fields or follow-up questions clearly when the backend returns them
- preserve backend review failures as actionable client errors
- avoid crashing on malformed responses
- remain usable for local scan/cache workflows when the backend is temporarily unavailable
- avoid leaking raw source content or secrets into logs

---

## Review pipeline boundary

Meridian's full review pipeline is backend-owned.

Conceptual backend review flow:

```text
Architecture model
    - review scope
    - user-provided context 
        → context sufficiency assessment 
        → domain and evidence coverage assessment 
        → readiness decision 
        → READY 
            → rules pre-pass 
            → signal brief 
            → prompt construction 
            → AI provider generation 
            → finding/report parsing 
            → decision-quality validation
            → rules post-validation 
            → final review report and review baseline 
        → PARTIAL 
            → return limitations and targeted questions 
            → optionally allow clearly marked preliminary review 
        → INSUFFICIENT 
            → do not produce a full review 
            → return missing context, rationale, and recommended questions
```

Conceptual backend intermediate review flow:

```text
Architecture model
    - file content or submitted change
    - prior full review baseline 
        → baseline lookup 
        → no baseline found 
            → recommend full review 
        → baseline found 
            → assess change against baseline 
            → detect drift, contradictions, new risks, decision impacts, or scope expansion 
            → return intermediate findings 
            → recommend another full review when change significance exceeds threshold
```

Conceptual backend decision and consensus flow:

```text
Recommendation or review finding
    → classify as architectural decision or supporting design
    → identify architectural stakeholders whose approval, signoff, or delegated authority is required
    → identify affected parties separately from approval stakeholders
    → identify represented concerns from affected parties
    → produce options, trade-offs, assumptions, consequences, and confidence
    → customer accountable decision authority accepts, rejects, modifies, defers, or supersedes
    → backend records decision outcome where supported
    → privacy-safe outcome feedback improves Meridian product intelligence
```

`meridian-mcp` only participates in:

```text
local scan 
    → local cache 
    → request relay 
    → response shaping
```

The local client must not become a second rules engine, readiness engine, baseline manager, decision register, consensus workflow, or prompt-construction runtime.

---

## Environment variables and local configuration

The current Meridian naming is:

- `MERIDIAN_API_KEY`
- `MERIDIAN_BACKEND_URL`
- `MERIDIAN_LOG`

The CLI may also support a local config file for values such as API key and backend URL.

---

## Binary constraints

### Must

- run as a single compiled binary
- include MCP server mode
- include CLI command mode
- write logs to stderr, not stdout
- keep stdout available for MCP JSON-RPC in MCP mode
- avoid direct AI provider SDK usage
- stay small and easy to distribute
- work without Node or JVM installed locally
- keep review, readiness, baseline, decision, billing, usage, and rule-governance authority backend-owned

### Should

- start quickly
- scan concurrently where useful
- degrade gracefully when files cannot be parsed
- skip irrelevant files aggressively
- provide clear CLI diagnostics
- keep terminal output readable in CLI mode
- preserve backend error messages in actionable form
- display backend-supplied missing-context questions and full-review recommendations clearly

### Must not

- embed product billing logic
- perform pricing calculations
- enforce usage policy locally
- persist user account data
- duplicate backend review policy
-
