# meridian-mcp TODO

Status key: `[x]` done, `[ ]` open. Decisions are in `ADR-AppArch-25`.

## Done

- [x] Rust crate scaffold (edition 2024) with Clap CLI and `serve` subcommand.
- [x] stdio MCP server on the official `rmcp` SDK; four tools listed and callable.
- [x] Backend HTTPS client (rustls), bearer API key, reply classification.
- [x] In-memory passphrase session; `passphrase_required` result tells the agent to ask the user.
- [x] Environment configuration (`MERIDIAN_API_KEY`, `MERIDIAN_API_URL`).

## Backend contract (blocks real use)

The v1 contract is `meridian-docs/architecture/mcp-api.md` (proposed). The backend work is tracked in
`meridian-backend/TODO.md`.

- [x] Define endpoints, headers, error codes, observation, fact, and question schemas (v1, proposed).
- [x] Settle that the passphrase travels in the `X-Architecture-Passphrase` header, unlocked inside
      each request, with `PASSPHRASE_REQUIRED`, `PASSPHRASE_INCORRECT`, and `PASSPHRASE_LOCKED`.
- [x] Align the client with the contract: `/projects/{id}/...` paths, headers (client ID, request
      ID, idempotency key, `If-Match`), and passphrase error codes.
- [x] Add `meridian_projects` (list and create); kept as a fifth tool rather than folded into `meridian_next`.
- [x] Add the `answers` field to `meridian_next` and the `If-Match` model version to writes.
- [x] Poll `GET .../evaluations/{id}` for `202` evaluation results (the v1 backend answers synchronously, so
      this path is not exercised yet).
- [x] Run the client against the real backend (2026-11-16, local jar, real API-key auth, encrypted store):
      passphrase required/incorrect, observations, inline answers, submit, model read-back, and
      `REVIEW_FULL` all worked. Not yet run against the Docker stack.
- [x] Add `meridian_projects` (list and create), tested against the local Docker backend.
- [ ] Handle `SERVICE_NOT_AVAILABLE` and `NOT_READY` evaluation results as clear tool output.
- [ ] Handle `PAYMENT_REQUIRED`, `RATE_LIMITED`, `CLIENT_UPGRADE_REQUIRED`, and `SERVICE_UNAVAILABLE`
      (honour `retryAfterSeconds`) as clear tool results.
- [ ] Settle whether API keys can be scoped to one project.

## Local scanning

- [ ] Repository walker (respect `.gitignore`, skip binaries and large files, bounded memory).
- [ ] Language and build-system detection; module, dependency, and entry-point extraction.
- [ ] Harvest architecture documents (ADRs, design docs, diagrams) as observations.
- [ ] Redaction: never emit secrets or raw source text; unit tests for the redaction rules.
- [ ] Incremental scans (changed files only) and a content hash so unchanged scans send nothing.
- [ ] Add a `scan` tool or fold scanning into `meridian_next` (decide; keep the tool surface small).

## MCP surface

- [ ] Set the server name and version (currently reports `rmcp`) and add usage instructions.
- [ ] Tool annotations (read-only versus writing) and clear descriptions for agent use.
- [ ] Structured output schemas for tool results, not just text.
- [ ] Decide on MCP resources or prompts (for example, the open-questions list) versus tools only.

## CLI and CI

- [ ] CLI subcommands that mirror the tools (`next`, `submit`, `evaluate`, `model`) with JSON output.
- [ ] CI mode: non-interactive run that fails on drift or unanswered required questions.
- [ ] Interactive passphrase prompt for CLI use (no echo); never accept it as a plain argument.
- [ ] `meridian-mcp doctor`: check configuration, reachability, and credentials.

## Security and privacy

- [ ] Hold the passphrase in zeroizing memory; never log it, an API key, or observation content.
- [ ] Restrict outbound calls to the configured backend host; pin TLS roots.
- [ ] Redact request bodies from all error messages and logs.
- [ ] Threat model note: the passphrase passes through the agent runtime and may appear in its
      transcript (accepted in `ADR-AppArch-25`); document the guidance for users.

## Build and distribution

- [ ] Cross-compile and test on Windows, macOS (x86_64, aarch64), and Linux (x86_64, aarch64).
- [ ] Release profile tuning (LTO, strip, small binary) and size budget.
- [ ] Code signing and notarization (Windows Authenticode, macOS notarization).
- [ ] Installers and update path (Homebrew, winget, direct download, `cargo install`).
- [ ] CI: build matrix, clippy, rustfmt, tests, dependency audit (`cargo audit`/`deny`).
- [ ] License file (Apache-2.0 per `meridian-docs/legal/README.md`) and `NOTICE`.

## Testing

- [ ] Unit tests for session, config, and reply classification.
- [ ] Integration tests against a mock backend (wiremock) covering `passphrase_required`, auth
      failure, entitlement denial, and timeouts.
- [ ] MCP conformance test: handshake, `tools/list`, and tool calls through a real MCP client.
- [ ] Scanner tests on fixture repositories.

## Documentation and distribution

- [ ] Real (anonymized) example session in this README once the backend is live.
- [ ] MCP registry listings (`meridian-docs/marketing/TODO.md`) after the first release build.
