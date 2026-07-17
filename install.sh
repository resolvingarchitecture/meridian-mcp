#!/usr/bin/env bash
set -euo pipefail

APP_NAME="meridian-mcp"
DOWNLOAD_BASE_URL="${DOWNLOAD_BASE_URL:-https://resolvingarchitecture.io/apps/meridian/downloads/meridian-mcp}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
INSTALLED_BIN="$INSTALL_DIR/$APP_NAME"

info() {
  printf '\033[1;34m[INFO]\033[0m %s\n' "$*"
}

warn() {
  printf '\033[1;33m[WARN]\033[0m %s\n' "$*"
}

error() {
  printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2
}

usage() {
  cat <<EOF
Meridian MCP installer

Usage:
  install.sh [options]

Options:
  --install-dir <dir>       Install directory. Default: \$HOME/.local/bin
  --download-base-url <url> Binary download base URL.
                            Default: https://resolvingarchitecture.io/apps/meridian/downloads/meridian-mcp
  --help                    Show this help message.

Environment overrides:
  INSTALL_DIR               Install directory.
  DOWNLOAD_BASE_URL         Binary download base URL.

Examples:
  curl -fsSL https://resolvingarchitecture.io/apps/meridian/downloads/meridian-mcp/install.sh | sh

  curl -fsSL https://resolvingarchitecture.io/apps/meridian/downloads/meridian-mcp/install.sh \\
    | sh -s -- --install-dir /usr/local/bin
EOF
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --install-dir)
        if [[ $# -lt 2 || -z "${2:-}" ]]; then
          error "--install-dir requires a value."
          exit 1
        fi
        INSTALL_DIR="$2"
        INSTALLED_BIN="$INSTALL_DIR/$APP_NAME"
        shift 2
        ;;
      --download-base-url)
        if [[ $# -lt 2 || -z "${2:-}" ]]; then
          error "--download-base-url requires a value."
          exit 1
        fi
        DOWNLOAD_BASE_URL="${2%/}"
        shift 2
        ;;
      --help|-h)
        usage
        exit 0
        ;;
      *)
        error "Unknown option: $1"
        usage
        exit 1
        ;;
    esac
  done
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    error "Required command not found: $1"
    return 1
  fi
}

detect_artifact_name() {
  local os
  local arch

  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64)
          printf '%s\n' "meridian-linux-x86_64"
          ;;
        *)
          error "Unsupported Linux architecture: $arch"
          error "Supported Linux architecture: x86_64"
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64)
          printf '%s\n' "meridian-macos-x86_64"
          ;;
        arm64|aarch64)
          printf '%s\n' "meridian-macos-arm64"
          ;;
        *)
          error "Unsupported macOS architecture: $arch"
          error "Supported macOS architectures: x86_64, arm64"
          exit 1
          ;;
      esac
      ;;
    *)
      error "Unsupported operating system: $os"
      error "This installer supports Linux and macOS."
      exit 1
      ;;
  esac
}

download_file() {
  local url="$1"
  local destination="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 3 --connect-timeout 15 --output "$destination" "$url"
    return 0
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -O "$destination" "$url"
    return 0
  fi

  error "Neither curl nor wget was found. Install one of them and re-run this installer."
  exit 1
}

ensure_path_hint() {
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
      warn "$INSTALL_DIR is not currently in your PATH."
      warn "Add this to your shell profile:"
      printf '\n  export PATH="%s:$PATH"\n\n' "$INSTALL_DIR"
      ;;
  esac
}

verify_installation() {
  if [[ ! -x "$INSTALLED_BIN" ]]; then
    error "Installed binary is not executable: $INSTALLED_BIN"
    exit 1
  fi

  info "Verifying installation..."

  if "$INSTALLED_BIN" version >/dev/null 2>&1; then
    "$INSTALLED_BIN" version || true
    return 0
  fi

  if "$INSTALLED_BIN" --version >/dev/null 2>&1; then
    "$INSTALLED_BIN" --version || true
    return 0
  fi

  warn "Installed binary did not respond to 'version' or '--version'."
  warn "Continuing because the binary was downloaded and marked executable."
}

main() {
  parse_args "$@"

  local artifact_name
  local download_url
  local temp_dir
  local temp_bin

  artifact_name="$(detect_artifact_name)"
  download_url="${DOWNLOAD_BASE_URL%/}/$artifact_name"
  temp_dir="$(mktemp -d)"
  temp_bin="$temp_dir/$artifact_name"

  trap 'rm -rf "$temp_dir"' EXIT

  info "Installing $APP_NAME..."
  info "Detected artifact: $artifact_name"
  info "Download URL: $download_url"

  require_command uname
  require_command mktemp
  require_command chmod
  require_command mkdir
  require_command cp

  info "Downloading binary..."
  download_file "$download_url" "$temp_bin"

  if [[ ! -s "$temp_bin" ]]; then
    error "Downloaded file is empty: $download_url"
    exit 1
  fi

  chmod +x "$temp_bin"

  info "Creating install directory: $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"

  info "Installing binary to: $INSTALLED_BIN"
  cp "$temp_bin" "$INSTALLED_BIN"
  chmod +x "$INSTALLED_BIN"

  verify_installation
  ensure_path_hint

  cat <<EOF

Installation complete.

Run:
  $APP_NAME help

Configure your API key:
  export MERIDIAN_API_KEY=<your-api-key>

Use in your MCP client config:
  command: $APP_NAME

EOF
}

main "$@"