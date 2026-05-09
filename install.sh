#!/usr/bin/env bash
set -euo pipefail

APP_NAME="meridian"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
BIN_PATH="$REPO_ROOT/target/release/$APP_NAME"
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

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    error "Required command not found: $1"
    return 1
  fi
}

install_rust_if_missing() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi

  warn "Rust/Cargo is not installed."

  if command -v rustup >/dev/null 2>&1; then
    info "rustup found. Installing stable Rust toolchain..."
    rustup toolchain install stable
    rustup default stable
    return 0
  fi

  if command -v curl >/dev/null 2>&1; then
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
    return 0
  fi

  error "Cargo is missing and curl is unavailable. Install Rust from https://rustup.rs/ and re-run this script."
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

main() {
  info "Installing $APP_NAME..."

  cd "$REPO_ROOT"

  if [[ ! -f "Cargo.toml" ]]; then
    error "Cargo.toml not found. Run this script from the project root."
    exit 1
  fi

  install_rust_if_missing
  require_command cargo

  info "Building release binary..."
  cargo build --release

  if [[ ! -x "$BIN_PATH" ]]; then
    error "Build completed, but binary was not found at: $BIN_PATH"
    exit 1
  fi

  info "Creating install directory: $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"

  info "Installing binary to: $INSTALLED_BIN"
  cp "$BIN_PATH" "$INSTALLED_BIN"
  chmod +x "$INSTALLED_BIN"

  info "Verifying installation..."
  "$INSTALLED_BIN" version || true

  ensure_path_hint

  cat <<EOF

Installation complete.

Run:
  meridian help

Configure your API key:
  meridian config set api-key <your-api-key>

Start MCP server:
  meridian mcp

EOF
}

main "$@"