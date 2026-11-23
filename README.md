# meridian-mcp

The agent-facing interface to Meridian. One native Rust binary for Windows, macOS, and Linux
that an agent talks to over MCP (stdio) and that talks to `meridian-backend` over HTTPS.

Decision record: `meridian-docs/architecture/adr/ADR-AppArch-25-Agent-Interview-Loop-Via-Rust-MCP.md`.
Backend counterpart: `meridian-backend/DESIGN.md` §4.14. Wire contract:
`meridian-docs/architecture/mcp-api.md`.

## What it does

People should not type architecture information into a UI and keep it current by hand. An agent
calls `meridian-mcp`; the backend works out where the project stands in designing and maintaining
its architecture and returns what is missing as questions. The agent answers from the repository
or asks the user, and the answers go back to the backend.

```text
agent --stdio MCP--> meridian-mcp --HTTPS--> meridian-backend
  1. meridian_next(project, observations)
  2. backend updates the completeness map and returns phase, gaps, and next questions
  3. agent answers from the repo or asks the user, then calls meridian_submit / meridian_next again
```

## Principles

- **Stateless client.** No local copy of the Architecture Model, no AI calls. The backend owns the
  model, the gap rules, entitlements, and payments.
- **Deterministic gap detection.** The backend's rules decide what is missing and what to ask; an
  LLM may only phrase questions. Same observations in, same questions out.
- **The connector scans locally and sends what the backend needs.** It sends structured observations
  (paths, counts, summaries), the text of architecture documents, and, as `SOURCE` observations, the
  source, test and build files a review needs as evidence of what is built. Source text is stored only in
  the project's Implementation Model, encrypted under the Architecture Passphrase, and is never logged.
- **Direct entry.** Facts submitted through the tools enter the model immediately, with provenance.
  There is no approval step.
- **The agent supplies the passphrase.** Tools that touch the model accept the Architecture
  Passphrase. When it is missing the tool returns `passphrase_required`; the agent asks the user
  and retries. The binary keeps it in process memory only and never writes it to disk or logs it.
- **One binary, three uses.** The MCP server, the CLI, and the CI step are the same executable.

## Tools

| Tool                | Purpose                                                                | Entitlement |
|---------------------|------------------------------------------------------------------------|-------------|
| `meridian_projects` | List your projects, or create one (the user chooses its passphrase)    | Free        |
| `meridian_next`     | Submit observations; get phase, gaps, inconsistencies, next questions  | Free        |
| `meridian_submit`   | Submit facts or answers with source and confidence                     | Free        |
| `meridian_evaluate` | Run an entitled service, or `quote_only` to see its price first        | Per service |
| `meridian_model`    | Read the current model, completeness map, findings, and decisions      | Free        |
| `meridian_account`  | Show your credits, the least a run costs, your usage and projects      | Free        |

### Review reports

A finished review comes back as a markdown `report`, and `meridian_evaluate` saves it straight away, so
nobody has to type a path. `save_to` overrides everything. Otherwise, from `repo_path` (default: the
server's working directory):

- inside a git repository, it goes in that repository's `reviews/` folder;
- if `repo_path` is not a repository but holds several, it goes in the `reviews/` folder of its
  architecture, design, or admin folder (a folder whose name contains `architecture`, then `design`, then `admin`);
- otherwise it goes in the `reviews/` folder of `repo_path` (or of the server's working directory).

`destinationReason` in the result says why a folder was chosen.

The file is named `<UTC time>-<service>-v<version>.md` (for example `2026-11-17T19-06-45Z-review_full-v1.3.1.md`), so
reports sort by time and those for different versions sit side by side, and starts with front matter that records the project's version at the time of the review
(`project_version`, and `version_source`), the Architecture Model version, the evaluation ID, and the time.
The version is read from `pom.xml`, `package.json`, `Cargo.toml`, or `pyproject.toml` in `repo_path`
(default: the server's working directory); pass `version` when the project has none of those. The result's
`saveError` says why a report was not saved.

Call `meridian_projects` first to get the `project` ID. Every other tool takes that `project` and an
optional `passphrase`; `meridian_projects` needs a passphrase only to create.

## Configuration

| Variable           | Purpose                                          | Default                                              |
|--------------------|--------------------------------------------------|------------------------------------------------------|
| `MERIDIAN_API_KEY` | API key created in `meridian-web` (never logged) | required                                             |
| `MERIDIAN_API_URL` | Backend base URL                                 | `https://resolvingarchitecture.io/apps/meridian/api` |

## Build and run

```bash
cd meridian-mcp
cargo build --release
MERIDIAN_API_KEY=... ./target/release/meridian-mcp serve   # MCP server on stdio (the default)
```

Register it with an MCP client by pointing the client at the binary:

```json
{
  "mcpServers": {
    "meridian": {
      "command": "/path/to/meridian-mcp",
      "args": ["serve"],
      "env": { "MERIDIAN_API_KEY": "..." }
    }
  }
}
```

## Download and release

The packages are built on real Linux, Windows, and macOS runners by `.github/workflows/release.yml`
(run it by hand, or push a `v*` tag), and each binary is smoke-tested on the OS it was built for
(`packaging/smoke.py`: `--version`, an MCP handshake, and the five tools).

| Package                                 | Built on     | Notes                                           |
|-----------------------------------------|--------------|-------------------------------------------------|
| `meridian-mcp-<version>-linux-x86_64`   | ubuntu-22.04 | `.tar.gz`; needs glibc 2.34 or newer            |
| `meridian-mcp-<version>-windows-x86_64` | windows-2022 | `.zip`; MSVC build                              |
| `meridian-mcp-<version>-macos-arm64`    | macos-14     | `.tar.gz`; Apple silicon                        |
| `meridian-mcp-<version>-macos-x86_64`   | macos-14     | `.tar.gz`; cross-compiled, tested under Rosetta |

To publish to the Meridian site: download the workflow's artifacts into `dist/`
(`gh run download <run-id> -D dist --pattern 'meridian-mcp-*'`), then run
`marketing/publish-meridian-mcp.sh` and `marketing/deploy_script.sh`. `./package.sh` builds and
smoke-tests a package for the current machine only. The builds are not code-signed or notarized.

## Layout

| Path             | Purpose                                                 |
|------------------|---------------------------------------------------------|
| `src/main.rs`    | CLI entry point (Clap) and stdio MCP server startup     |
| `src/tools.rs`   | The four MCP tools, each forwarding to the backend      |
| `src/backend.rs` | HTTPS client for `meridian-backend`; classifies replies |
| `src/session.rs` | In-memory passphrase holder (never persisted)           |
| `src/config.rs`  | Environment configuration                               |

## Status

Scaffold. The MCP handshake and the four tools work end to end against a stub; the backend
endpoints are specified in `meridian-docs/architecture/mcp-api.md` (v1, proposed) and not built
yet. See `TODO.md`.
