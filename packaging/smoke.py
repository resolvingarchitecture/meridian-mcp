#!/usr/bin/env python3
"""Smoke test for a meridian-mcp binary: --version, then an MCP handshake over stdio.

Usage: smoke.py PATH_TO_BINARY [EXPECTED_VERSION]
Exits non-zero on any failure. Needs no network and no credentials.
"""
import json
import os
import subprocess
import sys

EXPECTED_TOOLS = {
    "meridian_projects", "meridian_next", "meridian_submit", "meridian_model", "meridian_evaluate",
}


def fail(message):
    print(f"SMOKE TEST FAILED: {message}", file=sys.stderr)
    sys.exit(1)


def main():
    exe = sys.argv[1]
    version = subprocess.run([exe, "--version"], capture_output=True, text=True, timeout=30)
    if version.returncode != 0 or "meridian-mcp" not in version.stdout:
        fail(f"--version gave {version.returncode}: {version.stdout!r} {version.stderr!r}")
    reported = version.stdout.split()[-1]
    if len(sys.argv) > 2 and reported != sys.argv[2]:
        fail(f"version {reported} != expected {sys.argv[2]}")

    messages = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize",
         "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": {"name": "smoke", "version": "0"}}},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    ]
    env = dict(os.environ, MERIDIAN_API_KEY="smoke-test-key")
    run = subprocess.run([exe, "serve"], input="\n".join(json.dumps(m) for m in messages) + "\n",
                         capture_output=True, text=True, timeout=60, env=env)
    replies = {}
    for line in run.stdout.splitlines():
        try:
            reply = json.loads(line)
        except ValueError:
            continue
        if "id" in reply:
            replies[reply["id"]] = reply
    if 1 not in replies or 2 not in replies:
        fail(f"no handshake/tools reply; stdout={run.stdout[:300]!r} stderr={run.stderr[:300]!r}")
    info = replies[1]["result"]["serverInfo"]
    if info.get("name") != "meridian-mcp" or info.get("version") != reported:
        fail(f"serverInfo {info} does not match --version {reported}")
    tools = {t["name"] for t in replies[2]["result"]["tools"]}
    if tools != EXPECTED_TOOLS:
        fail(f"tools {sorted(tools)} != {sorted(EXPECTED_TOOLS)}")
    print(f"smoke test passed: meridian-mcp {reported}, {len(tools)} tools")


if __name__ == "__main__":
    main()
