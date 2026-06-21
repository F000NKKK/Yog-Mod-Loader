#!/usr/bin/env bash
# Build the Rust runtime and stage it where the Fabric dev client/server can
# find it on java.library.path.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Building Rust runtime (release)"
cargo build --release --manifest-path "$ROOT/rust/Cargo.toml"

case "$(uname -s)" in
    Linux*)  LIB="libyog_runtime.so" ;;
    Darwin*) LIB="libyog_runtime.dylib" ;;
    *)       LIB="yog_runtime.dll" ;;
esac

STAGE="$ROOT/fabric/run/natives"
mkdir -p "$STAGE"
cp "$ROOT/rust/target/release/$LIB" "$STAGE/"

echo "==> Staged $LIB in $STAGE"
echo "    Add to your Fabric run config JVM args:"
echo "        -Djava.library.path=$STAGE"
