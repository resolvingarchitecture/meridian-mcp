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

## High-level architecture
IDE / MCP client 
    → meridian-mcp (local Rust binary) 
    → scan repository 
    → cache Meridian locally 
    → on file review, send file + Meridian to backend 
    → backend validates API key 
    → backend enforces usage / tier limits 
    → backend builds prompt 
    → backend calls AI provider 
    → backend returns findings 
    ← meridian-mcp forwards findings to IDE

---

## Responsibilities

### `meridian-mcp`
- MCP server lifecycle
- tool registration
- filesystem scanning
- architecture model inference
- local cache read/write
- backend HTTP client
- response passthrough and error shaping

### Not `meridian-mcp`
- AI provider selection
- prompt assembly
- billing enforcement
- usage tracking
- API key issuance
- persistence of user/account data

---

## Internal modules

The current source layout is:

- `main.rs`
- `agent.rs`
- `cache.rs`
- `scanner/mod.rs`
- `scanner/walker.rs`
- `scanner/imports.rs`
- `scanner/patterns.rs`
- `scanner/adrs.rs`

### `main.rs`
Entry point and MCP server bootstrap.

Responsibilities:
- initialize logging
- load environment configuration
- register MCP tools
- dispatch requests to scanner / cache / agent modules

### `scanner/walker.rs`
Repository traversal and path collection.

Responsibilities:
- walk directories
- respect ignore rules
- identify candidate source and documentation files
- infer likely application layers from paths and names

### `scanner/imports.rs`
Dependency graph parsing and ordering.

Responsibilities:
- parse imports using tree-sitter grammars
- construct import graph
- derive dependency relationships
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

### `agent.rs`
Backend client.

Responsibilities:
- construct HTTP requests to backend
- include bearer auth header
- send file review payloads
- decode findings from backend responses
- return errors in a form MCP clients can surface cleanly

---

## MCP tools

The binary exposes the following tools:

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

## Architecture model

The architecture model is intentionally compact.

It should capture:
- inferred layers
- layer order
- architectural style
- repeated patterns
- ADR references
- root path
- scan timestamp

It should not try to be a complete code index.

### Design principle
The model should be good enough to inform a backend review prompt, not so large that it becomes expensive to compute or cache.

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

### Invalidation
Invalidate when:
- the directory structure changes
- the user explicitly invalidates
- the model is missing
- the cache appears stale

### Non-goals
- content-addressable indexing of every source file
- persistence across machines
- long-term historical model storage

---

## Backend integration

The backend is the source of truth for:
- authenticated access
- review policy
- tier usage enforcement
- prompt construction
- provider selection
- finding normalization

### Current backend contract
The binary should assume the backend exposes:
- `POST /api/review`
- `GET /api/usage`

### Request shape
The review request should include:
- architecture model
- file path
- file content
- project root or equivalent project identity fields as needed by the backend contract

### Authentication
Requests use:
- `Authorization: Bearer m_live_...`

### Error handling
The binary should:
- surface authentication failures clearly
- preserve backend review failures as actionable client errors
- avoid crashing on malformed responses
- remain usable even when the backend is temporarily unavailable

---

## Environment variables

The current Meridian naming is:

- `MERIDIAN_API_KEY`
- `MERIDIAN_BACKEND_URL`
- `MERIDIAN_LOG`

The codebase still contains some older ArchGuard naming in places, so migration should be treated as ongoing until all build, release, and configuration surfaces are updated consistently.

---

## Binary constraints

### Must
- run as a single compiled binary
- write logs to stderr, not stdout
- keep stdout available for MCP JSON-RPC
- avoid direct AI provider SDK usage
- stay small and easy to distribute

### Should
- start quickly
- scan concurrently where useful
- degrade gracefully when files cannot be parsed
- skip irrelevant files aggressively

### Must not
- embed product billing logic
- persist user account data
- duplicate backend review policy
- depend on Node or JVM runtime locally

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

The release artifacts and executable names should continue converging on Meridian naming.

---

## Design risks

### 1. Naming drift
Some repository artifacts still reflect the older ArchGuard naming. This can confuse users, release automation, and environment configuration.

### 2. Backend contract drift
If the backend request or auth schema changes, the binary must stay in lockstep.

### 3. Over-scanning
Too much scanning work during startup can hurt perceived performance. The scan model should remain intentionally light.

### 4. Local complexity creep
The binary should not absorb analysis logic that belongs in the backend.

### 5. Cache staleness
A stale model produces misleading review results. Invalidation should remain reliable and simple.

---

## Design boundaries for future work

Planned extensions should preserve the same split:
- `meridian-mcp`: local scanning + transport
- backend: intelligence + policy + billing
- IDE extension: presentation only

If future features add more local heuristics, they should still be shaped as:
- model enrichment
- not AI reasoning
- not policy enforcement
- not backend duplication

---

## Summary

`meridian-mcp` is a fast, auditable, local MCP bridge. Its architecture is intentionally narrow: scan, cache, and relay. Anything that looks like product logic should move to or stay in the backend.
