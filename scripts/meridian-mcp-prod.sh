#!/usr/bin/env bash
# Runs meridian-mcp against production Meridian, reading the API key from a file so the key never has to
# live in an MCP client's config. Create the key in the Meridian web app, then:
#   mkdir -p ~/.config/meridian && printf '%s' 'YOUR_KEY' > ~/.config/meridian/api-key && chmod 600 ~/.config/meridian/api-key
# Override the key file with MERIDIAN_API_KEY_FILE, or the binary with MERIDIAN_MCP_BIN.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KEYFILE="${MERIDIAN_API_KEY_FILE:-$HOME/.config/meridian/api-key}"
BIN="${MERIDIAN_MCP_BIN:-${HERE}/../target/release/meridian-mcp}"

if [[ ! -r "${KEYFILE}" ]]; then
  echo "meridian-mcp: no API key. Create one in the Meridian web app and save it to ${KEYFILE} (chmod 600)." >&2
  exit 1
fi
MERIDIAN_API_KEY="$(tr -d '[:space:]' < "${KEYFILE}")"
[[ -n "${MERIDIAN_API_KEY}" ]] || { echo "meridian-mcp: ${KEYFILE} is empty." >&2; exit 1; }
export MERIDIAN_API_KEY
export MERIDIAN_API_URL="${MERIDIAN_API_URL:-https://resolvingarchitecture.io/apps/meridian/api}"
exec "${BIN}" "$@"
