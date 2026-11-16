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

- [x] Set the server name and version (`meridian-mcp`) and add usage instructions for agents.
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

- [x] Workflow that builds and smoke-tests Linux x86_64, Windows x86_64, and macOS arm64 and x86_64 on real
      runners (`.github/workflows/release.yml`). Not run yet: it has to be pushed to GitHub first.
- [ ] Linux arm64 and Windows arm64 builds.
- [x] Release profile tuning (thin LTO, strip): the Linux binary is about 7.5 MB.
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

## Website download (2026-11-16)

- [x] Download section on `/meridian/` for Linux, Windows, and both macOS CPUs, with `SHA256SUMS`, setup
      steps, and the not-signed warning. `marketing/publish-meridian-mcp.sh` places the packages and
      `deploy_script.sh` refuses to deploy while the page links to files that are not there.
- [x] Default API URL fixed to `https://resolvingarchitecture.io/apps/meridian/api` (the old default did not
      match the production path).
- [ ] Run the release workflow, download the artifacts, publish, and deploy.
- [ ] Decide the license and ship a `LICENSE.txt` in each package (none is included yet).
- [ ] Ship `THIRD-PARTY-NOTICES.txt` (for example with `cargo-about`); the other downloads have one.
- [ ] Code-sign Windows (Authenticode) and sign and notarize macOS, so the warnings go away.
- [ ] Try the connector from real MCP clients (Claude Code, Cursor, and others); only a test driver has been used.

## Real-client test (2026-11-16)

- [x] Claude Code connected, loaded all five tools, listed projects, asked for the passphrase instead of guessing,
      and ran the interview loop against a local backend with no tool errors.
- [x] First run: observations and structured answers were rejected because the tool inputs were untyped and
      questions gave no answer shape. Fixed: typed tool schemas (allowed kinds, field names), optional
      observation ids (generated from kind and path), an `answerExample` and `answerFormat` on every question,
      and rejection messages that name the allowed values. Second run: nothing rejected, 7 turns instead of 12.
- [ ] Try other MCP clients (Cursor and others); only Claude Code has been used.
- [ ] Re-publish the downloads with these fixes (the published packages predate them).

