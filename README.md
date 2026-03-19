# archguard-mcp

Open-source MCP server for [ArchGuard](https://resolvingarchitecture.io/archguard) — real-time
architectural violation detection powered by AI.

## Install

```bash
# Via cargo
cargo install archguard-mcp

# Via install script (macOS / Linux)
curl -fsSL https://resolvingarchitecture.io/archguard/install.sh | sh

# Or download a binary from GitHub Releases
```

## Configure

Add to your MCP client config:

**Cursor** (`~/.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "archguard": {
      "command": "archguard-mcp",
      "env": {
        "ARCHGUARD_API_KEY": "ag_live_..."
      }
    }
  }
}
```

**Claude Code** (`~/.claude/mcp_servers.json`):
```json
{
  "archguard": {
    "command": "archguard-mcp",
    "env": {
      "ARCHGUARD_API_KEY": "ag_live_..."
    }
  }
}
```

Get your API key at [resolvingarchitecture.io/archguard/dashboard](https://resolvingarchitecture.io/archguard/dashboard).

## Tools

### `scan_project`
Scans a project directory and builds its architecture model.
Call once when opening a project — cached automatically.

```
root_dir: "/path/to/your/project"
```

### `review_file`
Reviews a single file for architectural violations.

```
root_dir:  "/path/to/your/project"
file_path: "/path/to/your/project/src/domain/Order.ts"
content:   "... file content ..."
```

Returns findings:
```json
{
  "findings": [{
    "severity":      "CRITICAL",
    "type":          "dependency_violation",
    "file":          "src/domain/Order.ts",
    "line":          42,
    "title":         "Domain importing from controller layer",
    "explanation":   "...",
    "consequence":   "...",
    "suggestion":    "...",
    "adr_reference": "ADR-004",
    "confidence":    0.92
  }]
}
```

### `invalidate_cache`
Clears the cached architecture model. Use after major refactors.

## Environment variables

| Variable | Required | Default                                          | Description                                             |
|---|---|--------------------------------------------------|---------------------------------------------------------|
| `ARCHGUARD_API_KEY` | Yes | —                                                | Your API key from resolvingarchitecture.io/archguard/dashboard |
| `ARCHGUARD_BACKEND_URL` | No | `https://resolvingarchitecture.io/archguard/api` | Backend URL                                             |
| `ARCHGUARD_LOG` | No | `archguard_mcp=info`                             | Log level (to stderr)                                   |

## Development

```bash
git clone https://github.com/resolvingarchitecture/archguard-mcp
cd archguard-mcp

# Build debug binary
cargo build

# Run tests
cargo test

# Test locally with Cursor (point at debug binary)
# In ~/.cursor/mcp.json:
# "command": "/path/to/archguard-mcp/target/debug/archguard-mcp"
# "env": { "ARCHGUARD_BACKEND_URL": "http://localhost:8080", ... }
```

## Architecture

```
src/
  main.rs           — MCP server, tool registration
  agent.rs          — HTTP call to Java backend
  cache.rs          — sled embedded cache (~/.cache/archguard/)
  scanner/
    mod.rs          — scan() entry point, ArchModel definition
    walker.rs       — file system traversal, layer inference
    imports.rs      — tree-sitter import graph + topological sort
    patterns.rs     — architectural pattern detection
    adrs.rs         — ADR and architecture doc harvesting
tests/
  scanner_test.rs   — integration tests with fixture projects
```

## Releasing

Push a tag to trigger the GitHub Actions release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Builds binaries for:
- Linux x86_64 (musl — runs anywhere)
- macOS x86_64
- macOS arm64 (Apple Silicon)
- Windows x86_64
