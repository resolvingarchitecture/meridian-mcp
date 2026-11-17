#!/usr/bin/env bash
# Runs meridian-mcp against the local Docker backend (meridian-infra/local, http://localhost:8080).
# The API key is read from ~/.config/meridian/api-key-local (override: MERIDIAN_API_KEY_FILE).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export MERIDIAN_API_KEY_FILE="${MERIDIAN_API_KEY_FILE:-$HOME/.config/meridian/api-key-local}"
export MERIDIAN_API_URL="${MERIDIAN_API_URL:-http://localhost:8080/api}"
exec "${HERE}/meridian-mcp-prod.sh" "$@"
