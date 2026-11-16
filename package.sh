#!/usr/bin/env bash
# Builds a release binary for THIS machine, smoke-tests it, and packages it into dist/.
#   ./package.sh
# Windows and macOS packages are built by .github/workflows/release.yml on real runners;
# marketing/publish-meridian-mcp.sh publishes whatever is in dist/.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
case "$(uname -s)" in
  Linux)  OS=linux ;;
  Darwin) OS=macos ;;
  *) echo "Unsupported OS for package.sh: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64 ;;
  arm64|aarch64) ARCH=arm64 ;;
  *) echo "Unsupported CPU: $(uname -m)" >&2; exit 1 ;;
esac

cargo build --release
python3 packaging/smoke.py target/release/meridian-mcp "${VERSION}"

NAME="meridian-mcp-${VERSION}-${OS}-${ARCH}"
STAGE="dist/${NAME}"
rm -rf "${STAGE}" "dist/${NAME}.tar.gz"
mkdir -p "${STAGE}"
cp target/release/meridian-mcp "${STAGE}/"
cp packaging/README.txt "${STAGE}/README.txt"
tar -C dist -czf "dist/${NAME}.tar.gz" "${NAME}"
rm -rf "${STAGE}"
(cd dist && sha256sum "${NAME}.tar.gz")
echo "Packaged dist/${NAME}.tar.gz"
