# meridian-mcp

Open-source MCP server for [Meridian](https://resolvingarchitecture.io/meridian) — real-time
architectural violation detection powered by AI.

## Install

```bash
# From source
cargo install --path .
```

```bash
# Via cargo
cargo install meridian-mcp

# Via install script (macOS / Linux)
curl -fsSL https://resolvingarchitecture.io/meridian/install.sh | sh

# Or download a binary from GitHub Releases
```

## Configure

Add to your MCP client config:

**Cursor** (`~/.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "meridian": {
      "command": "meridian-mcp",
      "env": {
        "MERIDIAN_API_KEY": "m_live_..."
      }
    }
  }
}
```

**Claude Code** (`~/.claude/mcp_servers.json`):
```json
{
  "meridian": {
    "command": "meridian-mcp",
    "env": {
      "MERIDIAN_API_KEY": "m_live_..."
    }
  }
}
```

Get your API key at [resolvingarchitecture.io/meridian](https://resolvingarchitecture.io/meridian).

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

| Variable               | Required | Default                                           | Description                                              |
|------------------------|----------|---------------------------------------------------|----------------------------------------------------------|
| `MERIDIAN_API_KEY`     | Yes      | —                                                 | Your API key from resolvingarchitecture.io/meridian      |
| `MERIDIAN_BACKEND_URL` | No       | `https://resolvingarchitecture.io/meridian/api`   | Backend URL                                              |
| `MERIDIAN_LOG`         | No       | `meridian_mcp=info`                               | Log level (to stderr)                                    |

## Development

```bash
git clone https://github.com/resolvingarchitecture/meridian-mcp
cd meridian-mcp

# Build debug binary
cargo build

# Run tests
cargo test

# Test locally with Cursor (point at debug binary)
# In ~/.cursor/mcp.json:
# "command": "/path/to/meridian-mcp/target/debug/meridian-mcp"
# "env": { "MERIDIAN_BACKEND_URL": "http://localhost:8080", ... }
```

## Architecture

```
src/
  main.rs           — MCP server, tool registration
  agent.rs          — HTTP call to Java backend
  cache.rs          — sled embedded cache (~/.cache/meridian/)
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
