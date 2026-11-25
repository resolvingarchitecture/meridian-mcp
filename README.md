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
  model, the gap rules, entitlements, and payments. The one thing kept locally is the account's credentials (below).
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
| `meridian_setup`    | Create an account, get an API key, buy credits with Bitcoin            | Free        |

### Local credentials

`meridian_setup` keeps what must not be lost between sessions: the account's API key, and, from the moment a pay-first
sign-up is created, its claim secret (the only way back into a sign-up whose payment has not arrived yet). They go
before anything is shown to the agent, so a crash or restart between "pay this" and "payment arrived" loses nothing.
The next start picks them up: `request_api_key` or `signup_status` resumes the sign-up, and the stored key is used
without `MERIDIAN_API_KEY` (a key in the environment wins).

| Platform | Folder (or `MERIDIAN_MCP_HOME`)                           |
|----------|-----------------------------------------------------------|
| Linux    | `$XDG_CONFIG_HOME/meridian-mcp`, else `~/.config/meridian-mcp` |
| macOS    | `~/Library/Application Support/meridian-mcp`              |
| Windows  | `%APPDATA%\meridian-mcp`                                  |

- **Default: a private file.** `credentials.json`, mode 0600 in a 0700 folder on Unix (Windows relies on the profile
  folder's permissions), written atomically, one entry per backend URL. This suits an agent with no password to
  remember. It is as exposed as an API key in an MCP configuration: anything running as the user can read it.
- **Opt in: encrypted.** Set `MERIDIAN_VAULT_PASSWORD` in this server's environment and the store is encrypted
  (`credentials.enc`: Argon2id and XChaCha20-Poly1305); an existing plaintext file is converted on the next write. A wrong or
  missing password is refused and never overwrites the file. The password is deliberately not a tool parameter, so it
  does not pass through the agent's context. It is separate from any Meridian account password (a pay-first account has
  none until claimed).
- **Not stored:** the Architecture Passphrase (memory only, as before) and any model content.
- **Not echoed:** the claim secret is never returned to the agent. The API key is returned once, when issued, because
  the web app's claim page needs it.
- **No keychain support.** Encryption at rest is the vault password above.

### Getting started without a key

The fully automatic path needs no email, password or captcha. `signup_with_bitcoin` (N credits) returns a Bitcoin
address and amount; the user pays from their own wallet; `signup_status` then waits for the payment, collects the API key
and starts using it. The first payment must be worth at least 10 USD; that is a minimum, not a price, and a Bitcoin payment of more buys more credits at the credit rate (rounded down). The account has no email or password until the person
opens the web app's `/claim` page and enters the key. The steps below are the email route.

`MERIDIAN_API_KEY` is optional. With no key the server still starts, and `meridian_setup` walks a new user through:

1. `create_account`: email, password, optional username. Meridian emails a verification link. Production sign-up is
   protected by a Cloudflare Turnstile check that only a browser can pass; if it refuses the call, the user signs up in
   the web app instead and continues from step 2.
2. `request_api_key`: signs in once (the password is used for that call and not kept), issues the key, and the server
   uses it from then on. The key is shown once; to keep it across sessions the user sets `MERIDIAN_API_KEY`.
   `replace_existing` revokes an existing key.
3. `buy_credits`: N credits, paid in Bitcoin. Returns the address, amount in sats, a `bitcoin:` URI and the expiry. The
   user pays from their own wallet; `purchase_status` reports `PAID` once it confirms.

### Review reports

A finished review comes back as a markdown `report`, and `meridian_evaluate` saves it straight away, so
nobody has to type a path. `save_to` overrides everything. Otherwise, from `repo_path` (default: the
server's working directory):

- inside a git repository, it goes in that repository's `reviews/` folder;
- if `repo_path` is not a repository but holds several, it goes in the `reviews/` folder of its
  architecture, design, or admin folder (a folder whose name contains `architecture`, then `design`, then `admin`);
- otherwise it goes in the `reviews/` folder of `repo_path` (or of the server's working directory).

Inside that `reviews/` folder, full reviews go in `full/` and change (intermediate) reviews in `intermed/`; a `save_to` you
give is used exactly as given, with no subfolder. `destinationReason` in the result says why a folder was chosen.

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
| `MERIDIAN_API_KEY` | API key from `meridian-web` or `meridian_setup` (never logged) | optional; wins over the stored key    |
| `MERIDIAN_MCP_HOME` | Folder for the local credential store                          | the user's config folder (below)       |
| `MERIDIAN_VAULT_PASSWORD` | Encrypts the credential store when set                    | unset: the file is not encrypted       |
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
| `src/tools.rs`   | The MCP tools, each forwarding to the backend           |
| `src/setup.rs`   | `meridian_setup`: account, API key, Bitcoin credits     |
| `src/backend.rs` | HTTPS client for `meridian-backend`; classifies replies |
| `src/session.rs` | In-memory passphrase holder (never persisted)           |
| `src/store.rs`   | Local credential store: private file, optional encryption |
| `src/config.rs`  | Environment configuration                               |

## Status

Scaffold. The MCP handshake and the four tools work end to end against a stub; the backend
endpoints are specified in `meridian-docs/architecture/mcp-api.md` (v1, proposed) and not built
yet. See `TODO.md`.
