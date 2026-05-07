# Meridian MCP — Design

## Purpose

`meridian-mcp` is the local, open-source MCP server that runs on a developer's machine.

Its job is to:

1. Scan a codebase and build a lightweight architecture model
2. Cache that model locally
3. Review saved files against the cached model
4. Forward review requests to the Meridian backend over HTTPS
5. Expose MCP tools to IDEs and MCP clients

It must stay thin. All architectural judgment, prompt construction, provider orchestration, billing enforcement, and persistence of product data belong in the backend.

---

## Design goals

- **Local-first execution**: run entirely on the developer machine as a standalone binary
- **Fast startup and scan**: produce a usable architecture model quickly
- **Minimal dependencies**: no Node runtime, no JVM, no external services required locally
- **Privacy-preserving**: only file content and model metadata needed for review are sent to the backend
- **MCP-compatible**: speak MCP over stdio and behave well with Cursor, Claude Code, and VS Code integrations
- **Backend-driven intelligence**: no AI calls from the binary itself
- **Stable caching**: avoid rescanning unchanged projects unnecessarily

---

## Current product role

The current Meridian architecture places the product intelligence in the backend, not in the MCP binary.

`meridian-mcp` is responsible for the local execution and the architecture model:
- walk the repository
- infer project structure
- parse imports
- detect architectural patterns
- harvest ADR references
- cache the result
- send file review payloads to the backend

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

The binary authenticates with a bearer API key using the `m_live_...` format.

---

## High-level flow
```text
IDE / MCP client 
    → meridian-mcp MCP server mode 
    → scan repository 
    → cache ArchModel locally 
    → on file review, send file + ArchModel to backend 
    → backend validates API key 
    → backend enforces usage and payment policy 
    → backend runs rules-assisted review 
    → backend builds prompt 
    → backend calls AI provider 
    → backend validates and normalizes findings 
    → backend returns findings 
    ← meridian-mcp forwards findings to IDE
    
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

### Not `meridian-mcp`

- AI provider selection
- direct AI provider calls
- deterministic rules execution for final product judgment
- prompt assembly
- billing enforcement
- pricing calculations
- usage tracking
- payment handling
- API key issuance
- session issuance
- user/account persistence
- customer-facing product data persistence
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
- return errors in a form MCP clients and CLI workflows can surface cleanly

---

## MCP tools

The binary exposes the following MCP tools:

### `scan_project(root_dir)`

Scans the repository and caches the architecture model.

Use when:

- a project is opened
- the user asks for a refresh
- the cache is missing or stale

### `review_file(root_dir, file_path, content)`

Reviews one file against the cached model.

Use when:

- a file is saved
- a user requests immediate feedback on the current file

This tool should:

- load the cached model
- if necessary, trigger or request a fresh scan
- forward the review request to the backend
- return findings to the client

### `invalidate_cache(root_dir)`

Drops the cached model for a project.

Use when:

- a major refactor changes structure
- the scan is clearly stale
- the user explicitly requests a rescan

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

The CLI should not introduce separate architecture judgment, separate backend contracts, or separate product policy.

---

## Architecture model

The architecture model is intentionally compact.

It should capture:

- project root
- inferred layers
- layer order
- inferred architectural style
- repeated patterns
- import graph summary
- dependency relationships relevant to architecture review
- ADR references
- architecture-document references
- scan timestamp
- cache key or freshness metadata

It should not try to be a complete code index.

### Design principle

The model should be good enough to ground backend review, not so large that it becomes expensive to compute, cache, transmit, or reason over.

The model should prefer structural facts over full source retention.

---

## Cache design

### Cache key

The current design uses a key derived from:

- project root
- directory structure fingerprint

### Cache behavior

- cache is local only
- cache lives under the user's home cache directory
- cache should be fast to read and write
- cache should be safe to discard and rebuild
- cache should not persist user account data
- cache should not become a long-term product data store

### Invalidation

Invalidate when:

- the directory structure changes
- the user explicitly invalidates
- the model is missing
- the cache appears stale
- a major refactor changes architecture-relevant structure

### Non-goals

- content-addressable indexing of every source file
- persistence across machines
- long-term historical model storage
- customer telemetry storage
- raw source-code retention for learning

---

## Backend integration

The backend is the source of truth for:

- authenticated access
- review policy
- usage enforcement
- payment and balance policy
- pricing policy
- deterministic rules-assisted review
- prompt construction
- provider selection
- finding normalization
- finding/report validation
- product data persistence

### Current backend package boundary

`meridian-backend` is a Java / Spring Boot modular monolith with feature-oriented packages.

The MCP binary should treat the backend as an HTTP API and should not duplicate backend package logic.

Relevant backend responsibilities include:

| Backend area  | Responsibility                                                           |
|---------------|--------------------------------------------------------------------------|
| `api`         | API-key issuance, validation, request filtering, authenticated context    |
| `security`    | Session issuance, validation, invalidation, and recovery flows           |
| `user`        | User account state, balances, API-key/session links                      |
| `payment`     | Payment orchestration, provider selection, reconciliation, and crediting |
| `usage`       | Append-only usage ledger and spend records                               |
| `pricingrule` | Deterministic pricing calculations                                       |
| `context`     | User-provided architecture context storage, validation, billing, usage   |
| `agent`       | AI provider abstraction and model invocation                             |
| `rules`       | Deterministic architecture rules and validation                          |
| `skills`      | Skill lifecycle and review-related flows                                 |
| `admin`       | Private administrative workflows and visibility                          |
| `infra`       | Shared database, AWS, and technical infrastructure                       |

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

- architecture model
- file path
- file content
- optional review scope
- optional user-provided architecture context
- project root or equivalent project identity fields as required by the backend contract

### Authentication

Requests use:

- `Authorization: Bearer <Meridian API key>`

The local client may also support local configuration helpers for storing or validating the API key, but API-key issuance and account state remain backend-owned.

### Error handling

The binary should:

- surface authentication failures clearly
- surface insufficient balance or usage-limit failures clearly
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
- file content or review scope
- user-provided context 
    → rules pre-pass 
    → signal brief 
    → prompt construction 
    → AI provider generation 
    → finding/report parsing 
    → rules post-validation 
    → final findings or report
```

`meridian-mcp` only participates in:

```text
local scan 
    → local cache 
    → request relay 
    → response shaping
```

The local client must not become a second rules engine or prompt-construction runtime.

---

## Environment variables and local configuration

The current Meridian naming is:

- `MERIDIAN_API_KEY`
- `MERIDIAN_BACKEND_URL`
- `MERIDIAN_LOG`

The CLI may also support a local config file for values such as API key and 
backend URL.

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

### Should

- start quickly
- scan concurrently where useful
- degrade gracefully when files cannot be parsed
- skip irrelevant files aggressively
- provide clear CLI diagnostics
- keep terminal output readable in CLI mode
- preserve backend error messages in actionable form

### Must not

- embed product billing logic
- perform pricing calculations
- enforce usage policy locally
- persist user account data
- duplicate backend review policy
- duplicate backend deterministic rules
- construct final review prompts
- call AI providers directly
- depend on Node or JVM runtime locally
- expose rule-mining functionality as a public MCP capability

---

## Rule Miner boundary

`meridian-rule-miner` is internal/admin infrastructure for improving the rule corpus.

It owns:

- external source registry
- source approval and license policy
- source profiling
- rule gap analysis
- mining plan generation
- budget-limited internal runs
- candidate rule extraction
- candidate normalization and deduplication
- candidate evidence
- rule lifecycle support
- governance workflows

`meridian-mcp` must not own or expose those workflows.

Rule Miner is:

- admin-only
- budget-limited
- license-aware
- source-attributed
- isolated from customer-facing review traffic
- unable to activate externally derived rules automatically

The local client may benefit from rules improved through backend governance, but it should not know or care whether a backend rule originated from customer feedback, internal analysis, or Rule Miner.

---

## Release and distribution

The release workflow builds platform binaries for:

- Linux x86_64
- macOS x86_64
- macOS arm64
- Windows x86_64

Distribution channels should remain:

- `cargo install`
- install script
- GitHub Releases

The release artifacts, executable names, command names, environment variables, and documentation should continue converging on Meridian naming.

---

## Design risks

### 1. Naming drift

Some repository artifacts still reflect older naming. This can confuse users, release automation, environment configuration, and CLI command discovery.

### 2. Backend contract drift

If backend review endpoints, auth schema, payment behavior, usage behavior, or response schemas change, the binary must stay in lockstep.

### 3. Over-scanning

Too much scanning work during startup can hurt perceived performance. The scan model should remain intentionally light.

### 4. Local complexity creep

The binary should not absorb analysis logic that belongs in the backend, including rules execution, prompt construction, billing policy, and usage policy.

### 5. Cache staleness

A stale model produces misleading review results. Invalidation should remain reliable and simple.

### 6. CLI / MCP behavior divergence

The CLI and MCP tools should reuse the same scanner, cache, and backend relay path. Divergent implementations would create inconsistent findings and support burden.

### 7. Protocol/logging mistakes

MCP mode requires stdout to remain protocol-safe. Logs and diagnostics must go to stderr or structured CLI output where appropriate.

---

## Design boundaries for future work

Planned extensions should preserve the same split:

- `meridian-mcp`: local scanning, local cache, MCP transport, CLI transport, backend relay
- `meridian-backend`: intelligence, rules, policy, billing, payments, usage, AI orchestration, product persistence
- `meridian-rule-miner`: internal rule improvement and governance
- IDE extensions: presentation and workflow integration only
- web application: account, payment, dashboard, report, and user-facing product surfaces

If future features add more local heuristics, they should still be shaped as:

- model enrichment
- local diagnostics
- transport improvements
- cache quality improvements

They should not become:

- AI reasoning
- final architectural judgment
- pricing or billing logic
- usage policy enforcement
- backend rules duplication
- prompt construction
- admin rule-mining behavior

---

## Summary

`meridian-mcp` is a fast, auditable, local Meridian client runtime. Its architecture is intentionally 
narrow: scan, cache, expose MCP tools, expose CLI commands, and relay review requests to the backend.

Anything that looks like product intelligence, customer policy, billing, payment, usage enforcement, 
AI orchestration, or rule governance should move to or stay in the backend and internal services.
