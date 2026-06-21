#!/usr/bin/env bash
# Yog build — a dotnet-style task runner.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

CONFIG="Release"   # Debug | Release
RUN_CLIENT=0

usage() {
    cat <<'EOF'
Yog build — a dotnet-style task runner.

Usage: ./build.sh <command> [target] [options]

Commands:
  restore           Fetch dependencies (cargo + gradle)
  build [target]    Compile (default target: all)
  run [target]      Build, then run the Fabric dev server (--client for client)
  test              Run tests (cargo test)
  clean             Remove build outputs and artifacts
  publish [target]  Release build + assemble artifacts/<target>/ (+ native/)

Targets:
  rust              Rust runtime (native lib)
  fabric            Fabric host mod (implies rust)   [default]
  all               Everything                       (build only)
  neoforge|forge    (not implemented yet — roadmap)

Options:
  -c, --configuration <Debug|Release>   default: Release
      --client                          run: launch dev client instead of server
  -h, --help

Examples:
  ./build.sh build
  ./build.sh run --client
  ./build.sh publish fabric
  ./build.sh test -c Debug
  ./build.sh clean
EOF
}

die() { echo "error: $*" >&2; exit 1; }

# ── toolchain helpers ───────────────────────────────────────────────────────

native_lib_name() {
    case "$(uname -s)" in
        Linux*)  echo "libyog_runtime.so" ;;
        Darwin*) echo "libyog_runtime.dylib" ;;
        *)       echo "yog_runtime.dll" ;;
    esac
}

# target/<profile> dir for the current configuration.
cargo_profile_dir() { [ "$CONFIG" = "Release" ] && echo release || echo debug; }

# Find a JDK 17 for the Gradle daemon (Gradle 8.8 can't run on Java 23+).
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

# Run a gradle task inside a loader dir on JDK 17.
gradle_in() {
    local dir="$1"; shift
    local jh
    find_java17 >/dev/null || die "JDK 17 not found (set YOG_JAVA17_HOME=/path/to/jdk17)"
    jh="$(find_java17)"
    ( cd "$ROOT/$dir" && JAVA_HOME="$jh" ./gradlew "$@" )
}

# ── build steps ─────────────────────────────────────────────────────────────

cargo_build() {
    local flag=""; [ "$CONFIG" = "Release" ] && flag="--release"
    cargo build $flag --manifest-path "$ROOT/rust/Cargo.toml"
}

# Stage the freshly built native lib for the Fabric dev runtime.
stage_native() {
    local lib; lib="$(native_lib_name)"
    mkdir -p "$ROOT/fabric/run/natives"
    cp "$ROOT/rust/target/$(cargo_profile_dir)/$lib" "$ROOT/fabric/run/natives/"
}

not_impl() { die "'$1' is not implemented yet (roadmap: Fabric -> NeoForge -> Forge)"; }

build_target() {
    case "$1" in
        rust)
            echo "==> build rust ($CONFIG)"
            cargo_build; stage_native ;;
        fabric)
            build_target rust
            echo "==> build fabric"
            gradle_in fabric build ;;
        all)
            build_target fabric ;;
        neoforge|forge) not_impl "$1" ;;
        *) die "unknown target: $1" ;;
    esac
}

# ── commands ────────────────────────────────────────────────────────────────

cmd_restore() {
    echo "==> restore: cargo fetch"
    cargo fetch --manifest-path "$ROOT/rust/Cargo.toml"
    echo "==> restore: gradle (resolve plugins/deps)"
    gradle_in fabric --quiet help
}

cmd_build() { build_target "${1:-all}"; }

cmd_run() {
    build_target rust
    if [ "$RUN_CLIENT" = 1 ]; then
        echo "==> run: Fabric dev client"
        gradle_in fabric runClient
    else
        echo "==> run: Fabric dev server"
        gradle_in fabric runServer
    fi
}

cmd_test() {
    echo "==> test: cargo test ($CONFIG)"
    local flag=""; [ "$CONFIG" = "Release" ] && flag="--release"
    cargo test $flag --manifest-path "$ROOT/rust/Cargo.toml"
}

cmd_clean() {
    echo "==> clean"
    cargo clean --manifest-path "$ROOT/rust/Cargo.toml" || true
    gradle_in fabric clean || true
    rm -rf "$ROOT/artifacts"
    echo "    removed rust/target, fabric/build, artifacts/"
}

cmd_publish() {
    local target="${1:-fabric}"
    [ "$target" = "all" ] && target="fabric"
    CONFIG="Release"
    build_target "$target"

    mkdir -p "$ROOT/artifacts/native"
    cp "$ROOT/rust/target/release/$(native_lib_name)" "$ROOT/artifacts/native/"

    local out="$ROOT/artifacts/$target"
    rm -rf "$out"; mkdir -p "$out"
    find "$ROOT/$target/build/libs" -maxdepth 1 -name '*.jar' \
        ! -name '*-dev.jar' ! -name '*-sources.jar' -exec cp {} "$out/" \; 2>/dev/null || true

    echo "==> published -> artifacts/$target/ (native in artifacts/native/)"
    ls -1 "$out" "$ROOT/artifacts/native" 2>/dev/null | sed 's/^/      /'
}

# ── dispatch ────────────────────────────────────────────────────────────────

[ $# -eq 0 ] && { usage; exit 0; }
cmd="$1"; shift

targets=()
while [ $# -gt 0 ]; do
    case "$1" in
        -c|--configuration)
            case "${2:-}" in
                [Dd]ebug)   CONFIG="Debug" ;;
                [Rr]elease) CONFIG="Release" ;;
                *) die "configuration must be Debug or Release" ;;
            esac
            shift 2 ;;
        --client) RUN_CLIENT=1; shift ;;
        --server) RUN_CLIENT=0; shift ;;
        -h|--help) usage; exit 0 ;;
        -*) die "unknown option: $1" ;;
        *)  targets+=("$1"); shift ;;
    esac
done

target="${targets[0]:-}"

case "$cmd" in
    restore)        cmd_restore ;;
    build)          cmd_build "${target:-all}" ;;
    run)            cmd_run ;;
    test)           cmd_test ;;
    clean)          cmd_clean ;;
    publish)        cmd_publish "${target:-fabric}" ;;
    -h|--help|help) usage ;;
    *) echo "unknown command: $cmd" >&2; usage; exit 2 ;;
esac
