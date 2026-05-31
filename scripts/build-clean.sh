#!/bin/bash
# Produce a release build with no trace of the local machine's identity.
#
# Two leak sources are handled:
#   1. Dependency source paths ($HOME/.cargo/...) — stripped via --remap-path-prefix.
#   2. The project path embedded by Tauri's generate_context! macro — neutralised by
#      building from a username-free staging directory.
#
# The real distribution builds run in CI (GitHub runner paths contain no username),
# but this gives an equally clean artifact locally.
set -euo pipefail

SRC="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="${TMPDIR:-/tmp}/wavecheck-build"

echo "Staging source to $STAGE (no username in path)…"
rm -rf "$STAGE"
mkdir -p "$STAGE"
rsync -a --exclude target --exclude node_modules --exclude build \
      --exclude .svelte-kit --exclude .git "$SRC/" "$STAGE/"

cd "$STAGE"
echo "Installing deps + building frontend…"
npm install --silent
npm run build

echo "Building signed-path-free release bundle…"
# Remap any remaining absolute home paths out of the binary.
export RUSTFLAGS="--remap-path-prefix=$HOME=/b ${RUSTFLAGS:-}"
npm run tauri build

echo ""
echo "Artifacts: $STAGE/src-tauri/target/release/bundle/"
echo "Verify with:  strings -a \"$STAGE/src-tauri/target/release/wavecheck\" | grep -i \"$(whoami)\"  # expect no output"
