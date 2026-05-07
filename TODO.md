# TODO

## MVP

### Application
- [x] Implement core MCP tools: `scan_project`, `review_file`, and `invalidate_cache`
- [x] Keep the binary thin; do not move AI logic, prompt construction, provider orchestration, billing logic, or usage policy into MCP
- [x] Keep stdout reserved for MCP JSON-RPC and write logs/diagnostics to stderr
- [x] Reuse scanner, cache, and backend relay path for review flow
- [x] Replace remaining ArchGuard naming in code, packaging, configs, cache paths, logs, and user-facing messages with Meridian naming
- [x] Rename server/runtime identifiers from ArchGuard-oriented names to Meridian-oriented names
- [x] Align MCP server metadata, binary name, command name, and release artifacts with Meridian branding
- [x] Add CLI command dispatch while reusing the same scanner, cache, and backend client modules
- [x] Implement or wire recommended CLI commands:
    - [x `meridian mcp`
    - [x] `meridian scan [root_dir]`
    - [x] `meridian review <file_path>`
    - [x] `meridian cache clear [root_dir]`
    - [x] `meridian config set api-key <key>`
    - [x] `meridian doctor`
    - [x] `meridian version`
- [ ] Improve error messages when backend auth fails, balance/usage policy blocks review, or the backend is unreachable
- [ ] Add end-to-end smoke tests for `scan_project`, `review_file`, and `invalidate_cache`
- [ ] Validate scan performance on large repositories
- [ ] Add fixture coverage for layered, DDD, clean architecture, hexagonal, and mixed-structure projects
- [ ] Make cache invalidation more explicit and observable
- [ ] Add local diagnostics for configuration, backend reachability, and cache health

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
- [ ] Add cache key or freshness metadata to the architecture model or review payload if needed
- [ ] Confirm what minimum metadata the backend needs for review fidelity
- [ ] Ensure file paths are normalized consistently before review requests are sent
- [ ] Ensure scan model remains compact and does not become a full source index

### Cache
- [x] Persist architecture model locally
- [x] Load cached model for review
- [x] Allow explicit cache invalidation
- [x] Derive cache key from project root and directory structure fingerprint
- [x] Avoid storing raw source content in the cache
- [ ] Rename local cache directory from old product naming to Meridian naming
- [ ] Confirm cache location and cache key strategy are stable across supported platforms
- [ ] Track scan freshness metadata more explicitly
- [ ] Add cache health diagnostics for `meridian doctor`
- [ ] Validate cache file permissions on Linux, macOS, and Windows

### Backend Integration
- [x] Use bearer authentication for backend requests
- [x] Send file review payloads to the backend instead of performing local AI review
- [x] Preserve backend error body for non-success responses
- [ ] Replace old environment variable names with:
    - [ ] `MERIDIAN_API_KEY`
    - [ ] `MERIDIAN_BACKEND_URL`
    - [ ] `MERIDIAN_LOG`
- [ ] Optionally support old ArchGuard environment variable names temporarily as migration aliases, with warnings
- [ ] Confirm API key validation and messages align with `m_live_...` Meridian API keys
- [ ] Replace generic `/api/review` usage with the backend's current review endpoint
- [ ] Confirm review endpoint selection among:
    - [ ] `POST /api/skills/review/full`
    - [ ] `POST /api/skills/review/intermediate`
    - [ ] `POST /api/skills/review/full/prompt`
- [ ] Confirm the review request payload matches the selected backend endpoint exactly
- [ ] Confirm the review response shape maps cleanly to MCP findings
- [ ] Surface authentication failures clearly
- [ ] Surface insufficient balance or usage-limit failures clearly
- [ ] Avoid crashing on malformed backend responses
- [ ] Add resilient retry behavior for transient backend failures where appropriate
- [ ] Verify usage, tier, pricing, payment, and balance enforcement remain backend-only

### MCP
- [x] Expose `scan_project(root_dir)`
- [x] Expose `review_file(root_dir, file_path, content)`
- [x] Expose `invalidate_cache(root_dir)`
- [ ] Harden MCP tool input validation
- [ ] Confirm MCP tool descriptions match Meridian naming and current behavior
- [ ] Ensure MCP mode startup does not print non-protocol output to stdout
- [ ] Document local configuration for Cursor, Claude Code, and VS Code MCP usage
- [ ] Add compatibility tests for MCP request/response behavior

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

### Documentation
- [x] Update design direction around Meridian/backend-owned product intelligence
- [x] Document current intended module boundaries
- [x] Document current backend API surface
- [x] Document MCP tool responsibilities
- [x] Document cache design and non-goals
- [x] Document Rule Miner boundary as out of scope for MCP
- [ ] Update README to match Meridian naming and current `DESIGN.md`
- [ ] Update setup instructions for `MERIDIAN_API_KEY`, `MERIDIAN_BACKEND_URL`, and `MERIDIAN_LOG`
- [ ] Document CLI commands once implemented
- [ ] Document migration path from older ArchGuard naming
- [ ] Document backend endpoint compatibility expectations

### Security and Privacy
- [x] Avoid direct AI provider SDK usage in the local binary
- [x] Avoid local billing, pricing, payment, usage, account, or product-data persistence
- [x] Avoid storing raw source code in the architecture cache
- [ ] Audit logging to ensure file contents, API keys, and secrets are never logged
- [ ] Confirm backend auth failures are surfaced without leaking sensitive details
- [ ] Review review-payload construction to ensure only necessary file content and model metadata are sent
- [ ] Consider optional local redaction controls before review requests are sent
- [ ] Confirm local config storage does not expose API keys unnecessarily

## Backlog

### Application
- [ ] Improve architecture pattern detection heuristics
- [ ] Expand fixture coverage for language edge cases and malformed syntax
- [ ] Add richer local diagnostics without adding product judgment locally
- [ ] Add startup and scan timing metrics for local diagnostics
- [ ] Improve terminal output formatting for CLI mode

### Scanner and Architecture Model
- [ ] Add richer scan metadata if the backend starts using it
- [ ] Improve import parsing for Rust and Go if backend review benefits from it
- [ ] Improve topological ordering behavior for cycles and ambiguous dependencies
- [ ] Record lightweight scan diagnostics for debugging stale model issues
- [ ] Add architecture-document metadata beyond first-heading summaries if needed

### Integration
- [ ] Add support for future backend review fields without breaking compatibility
- [ ] Add explicit compatibility tests against backend API revisions
- [ ] Consider a migration helper for users upgrading from ArchGuard naming
- [ ] Add a backend capability/version check if the API introduces one

### Infrastructure
- [ ] Add benchmark automation for scan and startup latency
- [ ] Add CI checks for MCP protocol-safe stdout behavior
- [ ] Add CI checks to prevent reintroducing old product naming
- [ ] Add release smoke tests for generated binaries

### Security
- [ ] Validate cache file permissions on all supported platforms
- [ ] Add tests proving logs do not include file content or API keys
- [ ] Consider config-file encryption or OS keychain support for stored API keys