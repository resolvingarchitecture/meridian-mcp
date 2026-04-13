# TODO

## MVP

### Application
- [ ] Replace remaining ArchGuard naming in code, packaging, and configs with Meridian naming
- [ ] Align MCP server metadata, binary name, and release artifacts with Meridian branding
- [ ] Finish MCP tool contract hardening against the current backend request/response shape
- [ ] Add end-to-end smoke tests for `scan_project`, `review_file`, and `invalidate_cache`
- [ ] Improve error messages when backend auth fails or the backend is unreachable
- [ ] Verify `MERIDIAN_API_KEY` and `MERIDIAN_BACKEND_URL` are used consistently everywhere
- [ ] Keep the binary thin; do not move AI logic or billing logic into MCP
- [ ] Validate scan performance on large repositories
- [ ] Add fixture coverage for layered, DDD, and mixed-structure projects
- [ ] Make cache invalidation more explicit and observable

### Data
- [ ] Define and document the exact `ArchModel` JSON contract used by the backend
- [ ] Confirm how ADR references are represented in the scan output
- [ ] Confirm what minimum metadata the backend needs for review fidelity
- [ ] Ensure file paths are normalized consistently before review requests are sent

### Integration
- [ ] Confirm the request payload matches the backend `/api/review` endpoint exactly
- [ ] Confirm the API key format and auth header behavior remain `m_live_...` compatible
- [ ] Verify usage / tier enforcement is handled only by the backend
- [ ] Add a resilient retry strategy for transient backend failures where appropriate
- [ ] Document the local config required for Cursor, Claude Code, and VS Code usage

### Infrastructure
- [ ] Ensure release workflow emits Meridian-named artifacts
- [ ] Verify Linux, macOS Intel, macOS ARM, and Windows release targets still build
- [ ] Confirm cache location and cache key strategy are stable
- [ ] Update install script references to Meridian endpoints and names
- [ ] Remove old ArchGuard references from release and packaging automation

### Security
- [ ] Audit logging to ensure file contents and API keys are never logged
- [ ] Keep stdout reserved exclusively for MCP JSON-RPC
- [ ] Confirm backend auth failures are surfaced without leaking sensitive details
- [ ] Review local cache contents to ensure no code content is stored

## Backlog

### Application
- [ ] Add richer scan metadata if the backend starts using it
- [ ] Improve architecture pattern detection heuristics
- [ ] Add a command-line mode if it can reuse the same MCP core safely
- [ ] Expand fixture coverage for language edge cases and malformed syntax

### Data
- [ ] Track scan freshness metrics in cache metadata
- [ ] Record lightweight scan diagnostics for debugging stale model issues

### Integration
- [ ] Add support for any future backend review fields without breaking compatibility
- [ ] Add explicit compatibility tests against backend API revisions
- [ ] Consider a migration helper for users upgrading from ArchGuard naming

### Infrastructure
- [ ] Publish a clearer release matrix in the README and release workflow
- [ ] Add benchmark automation for scan and startup latency

### Security
- [ ] Consider optional local redaction controls before review requests are sent
- [ ] Validate cache file permissions on all supported platforms