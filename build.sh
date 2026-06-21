#!/usr/bin/env bash
# Yog build helper.
#
#   ./build.sh [component...]
#
# Components:
#   rust | cargo   Build the Rust runtime (release) and stage the native lib
#   fabric         Build the Fabric host mod -> artifacts/fabric/
#   run            Run the Fabric dev server (depends on rust)
#   neoforge       (not implemented yet — roadmap)
#   forge          (not implemented yet — roadmap)
#   all            Build everything available (rust + fabric)
#
# No args defaults to: rust
#
# Build outputs are copied into artifacts/<loader>/. The Gradle parts auto-pick a
# JDK 17 (Gradle 8.8 can't run on Java 23+); override with YOG_JAVA17_HOME=...
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() { sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; }

# ── helpers ────────────────────────────────────────────────────────────────

native_lib_name() {
    case "$(uname -s)" in
        Linux*)  echo "libyog_runtime.so" ;;
        Darwin*) echo "libyog_runtime.dylib" ;;
        *)       echo "yog_runtime.dll" ;;
    esac
}

# Find a JDK 17 for the Gradle daemon.
find_java17() {
    if [ -n "${YOG_JAVA17_HOME:-}" ] && [ -x "${YOG_JAVA17_HOME}/bin/java" ]; then
        echo "$YOG_JAVA17_HOME"; return 0
    fi
    for d in /usr/lib/jvm/java-17-openjdk-amd64 \
             /usr/lib/jvm/java-1.17.0-openjdk-amd64 \
             /usr/lib/jvm/openjdk-17 \
             "$HOME"/.sdkman/candidates/java/17*; do
        [ -x "$d/bin/java" ] && { echo "$d"; return 0; }
    done
    return 1
}

build_rust() {
    echo "==> Building Rust runtime (release)"
    cargo build --release --manifest-path "$ROOT/rust/Cargo.toml"
    local lib; lib="$(native_lib_name)"
    local src="$ROOT/rust/target/release/$lib"
    # Stage for the Fabric dev runtime (java.library.path).
    mkdir -p "$ROOT/fabric/run/natives"
    cp "$src" "$ROOT/fabric/run/natives/"
    echo "    Staged $lib -> fabric/run/natives"
    # Loader-agnostic artifact: one shared copy, not duplicated per loader.
    mkdir -p "$ROOT/artifacts/native"
    cp "$src" "$ROOT/artifacts/native/"
    echo "    Artifact -> artifacts/native/$lib"
}

# Run a gradle task inside a loader dir on JDK 17.
gradle_in() {
    local dir="$1"; shift
    local jh
    if ! jh="$(find_java17)"; then
        echo "!! JDK 17 not found (Gradle 8.8 can't run on Java 23+)." >&2
        echo "   Set YOG_JAVA17_HOME=/path/to/jdk17 and retry." >&2
        return 1
    fi
    echo "==> [$dir] JAVA_HOME=$jh ./gradlew $*"
    ( cd "$ROOT/$dir" && JAVA_HOME="$jh" ./gradlew "$@" )
}

# Copy the distributable jar(s) into artifacts/<loader>/ (native lib is shared,
# see build_rust -> artifacts/native/).
collect_artifacts() {
    local loader="$1"
    local out="$ROOT/artifacts/$loader"
    rm -rf "$out"; mkdir -p "$out"
    # remapped distributable jar(s), excluding dev/sources builds
    find "$ROOT/$loader/build/libs" -maxdepth 1 -name '*.jar' \
        ! -name '*-dev.jar' ! -name '*-sources.jar' -exec cp {} "$out/" \; 2>/dev/null || true
    echo "    Artifacts -> $out  (native lib in artifacts/native/)"
    ls -1 "$out" 2>/dev/null | sed 's/^/      /'
}

build_fabric() {
    build_rust
    gradle_in fabric build
    collect_artifacts fabric
}

not_impl() {
    echo "==> '$1' is not implemented yet (roadmap: Fabric -> NeoForge -> Forge)."
}

# ── dispatch ───────────────────────────────────────────────────────────────

[ $# -eq 0 ] && set -- rust

for comp in "$@"; do
    case "$comp" in
        rust|cargo)     build_rust ;;
        fabric)         build_fabric ;;
        run)            build_rust; gradle_in fabric runServer ;;
        neoforge|forge) not_impl "$comp" ;;
        all)            build_fabric ;;
        -h|--help|help) usage ;;
        *) echo "Unknown component: $comp" >&2; usage; exit 2 ;;
    esac
done
