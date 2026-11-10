#!/usr/bin/env bash
# Build the demo site into web/dist. Works locally and on Vercel.
#
# Needs: rustup (installed if missing), node.
set -euo pipefail

WASM_BINDGEN_VERSION=0.2.100
here="$(cd "$(dirname "$0")" && pwd)"
root="$(dirname "$here")"

if ! command -v cargo >/dev/null 2>&1; then
  [ -f "$HOME/.cargo/env" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  source "$HOME/.cargo/env"
fi
rustup target add wasm32-unknown-unknown

if [ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" != "$WASM_BINDGEN_VERSION" ]; then
  if [ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ]; then
    name="wasm-bindgen-$WASM_BINDGEN_VERSION-x86_64-unknown-linux-musl"
    curl -sSfL "https://github.com/rustwasm/wasm-bindgen/releases/download/$WASM_BINDGEN_VERSION/$name.tar.gz" \
      | tar -xz -C /tmp
    export PATH="/tmp/$name:$PATH"
  else
    cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked
  fi
fi

cargo build --release --target wasm32-unknown-unknown --manifest-path "$here/wasm/Cargo.toml"
wasm-bindgen --target web --out-dir "$here/src/pkg" \
  "$here/wasm/target/wasm32-unknown-unknown/release/edi_parse_wasm.wasm"

mkdir -p "$here/public"
node "$here/make-fs.mjs" "$root" > "$here/public/fs.json"

cd "$here"
npm ci
npm run build
echo "built $here/dist"
