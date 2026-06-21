#!/usr/bin/env bash
# Yog build helper.
#
#   ./build.sh [component...]
#
# Components:
#   rust | cargo   Build the Rust runtime (release) and stage the native lib
#   fabric         Build the Fabric host mod (depends on rust)
#   run            Run the Fabric dev server (depends on rust)
#   neoforge       (not implemented yet — roadmap)
#   forge          (not implemented yet — roadmap)
#   all            Build everything available (rust + fabric)
#
# No args defaults to: rust
#
# The Gradle parts auto-pick a JDK 17 (Gradle 8.8 can't run on Java 23+).
# Override detection with: YOG_JAVA17_HOME=/path/to/jdk17
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() { sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; }

# ── helpers ────────────────────────────────────────────────────────────────

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
    case "$(uname -s)" in
        Linux*)  lib="libyog_runtime.so" ;;
        Darwin*) lib="libyog_runtime.dylib" ;;
        *)       lib="yog_runtime.dll" ;;
    esac
    local stage="$ROOT/fabric/run/natives"
    mkdir -p "$stage"
    cp "$ROOT/rust/target/release/$lib" "$stage/"
    echo "    Staged $lib -> $stage"
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

not_impl() {
    echo "==> '$1' is not implemented yet (roadmap: Fabric -> NeoForge -> Forge)."
}

# ── dispatch ───────────────────────────────────────────────────────────────

[ $# -eq 0 ] && set -- rust

for comp in "$@"; do
    case "$comp" in
        rust|cargo)     build_rust ;;
        fabric)         build_rust; gradle_in fabric build ;;
        run)            build_rust; gradle_in fabric runServer ;;
        neoforge|forge) not_impl "$comp" ;;
        all)            build_rust; gradle_in fabric build ;;
        -h|--help|help) usage ;;
        *) echo "Unknown component: $comp" >&2; usage; exit 2 ;;
    esac
done
