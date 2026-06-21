#!/usr/bin/env bash
# Yog build — a dotnet-style task runner.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Implemented loaders. Add 'neoforge' / 'forge' here when their hosts land.
LOADERS=(fabric)

CONFIG="Release"   # Debug | Release
RUN_CLIENT=0

usage() {
    cat <<'EOF'
Yog build — a dotnet-style task runner.

Usage: ./build.sh <command> [target] [options]

Commands:
  restore             Fetch dependencies (cargo + every implemented loader)
  build [target]      Compile (default target: all)
  run <loader>        Build, then run that loader's dev server (--client for client)
  test                Run tests (cargo test)
  clean               Remove build outputs and artifacts
  publish <target>    Release build + assemble artifacts/<loader>/ (+ native/)

Targets:
  rust                Rust runtime (native lib)
  <loader>            One of the implemented loaders (currently: fabric)
  all                 rust + every implemented loader

Loader-bound commands (run, publish) require an explicit loader — there is no
default loader, since Yog targets many (fabric, neoforge, forge, ...).

Options:
  -c, --configuration <Debug|Release>   default: Release
      --client                          run: launch dev client instead of server
  -h, --help

Examples:
  ./build.sh build
  ./build.sh run fabric --client
  ./build.sh publish fabric
  ./build.sh publish all
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

# Platform tag matching the Rust runtime / Java host, e.g. linux-x86_64.
platform_tag() {
    local os arch
    case "$(uname -s)" in Linux*) os=linux ;; Darwin*) os=macos ;; *) os=windows ;; esac
    case "$(uname -m)" in
        x86_64|amd64)   arch=x86_64 ;;
        aarch64|arm64)  arch=aarch64 ;;
        *)              arch="$(uname -m)" ;;
    esac
    echo "${os}-${arch}"
}

cargo_profile_dir() { [ "$CONFIG" = "Release" ] && echo release || echo debug; }

is_loader() {
    local l
    for l in "${LOADERS[@]}"; do [ "$l" = "$1" ] && return 0; done
    return 1
}

# Resolve a loader target, failing helpfully for unimplemented / unknown ones.
require_loader() {
    [ -n "${1:-}" ] || die "this command needs a loader (one of: ${LOADERS[*]})"
    if ! is_loader "$1"; then
        case "$1" in
            neoforge|forge) die "'$1' is not implemented yet (roadmap)" ;;
            *) die "unknown loader: '$1' (have: ${LOADERS[*]})" ;;
        esac
    fi
}

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

build_rust() {
    echo "==> build rust ($CONFIG)"
    cargo_build
    local lib src tag l
    lib="$(native_lib_name)"
    src="$ROOT/rust/target/$(cargo_profile_dir)/$lib"
    tag="$(platform_tag)"
    # Embed the runtime native into each loader's jar resources so the jar is
    # self-contained — players never handle a loose .so/.dll.
    for l in "${LOADERS[@]}"; do
        mkdir -p "$ROOT/$l/src/main/resources/natives/$tag"
        cp "$src" "$ROOT/$l/src/main/resources/natives/$tag/"
    done
    echo "    embedded $lib into loader resources ($tag)"
}

# Build the example mod into a .yog and stage it in each loader's dev mods dir.
build_example() {
    echo "==> build example-mod (.yog)"
    cargo build --release -p yog-cli --manifest-path "$ROOT/rust/Cargo.toml"
    ( cd "$ROOT/example-mod" && "$ROOT/rust/target/release/yog" build )
    local l
    for l in "${LOADERS[@]}"; do
        mkdir -p "$ROOT/$l/run/yog-mods"
        cp "$ROOT/example-mod/artifacts/"*.yog "$ROOT/$l/run/yog-mods/" 2>/dev/null || true
    done
}

build_loader() {
    echo "==> build $1"
    gradle_in "$1" build
}

build_target() {
    case "$1" in
        rust) build_rust ;;
        all)  build_rust; build_example; local l; for l in "${LOADERS[@]}"; do build_loader "$l"; done ;;
        *)    require_loader "$1"; build_rust; build_loader "$1" ;;
    esac
}

# ── commands ────────────────────────────────────────────────────────────────

cmd_restore() {
    echo "==> restore: cargo fetch"
    cargo fetch --manifest-path "$ROOT/rust/Cargo.toml"
    local l
    for l in "${LOADERS[@]}"; do
        echo "==> restore: $l (resolve plugins/deps)"
        gradle_in "$l" --quiet help
    done
}

cmd_build() { build_target "${1:-all}"; }

cmd_run() {
    require_loader "${1:-}"
    local loader="$1"
    build_rust
    build_example
    if [ "$RUN_CLIENT" = 1 ]; then
        echo "==> run: $loader dev client"
        gradle_in "$loader" runClient
    else
        echo "==> run: $loader dev server"
        gradle_in "$loader" runServer
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
    local l
    for l in "${LOADERS[@]}"; do gradle_in "$l" clean || true; done
    rm -rf "$ROOT/artifacts"
    echo "    removed rust/target, <loader>/build, artifacts/"
}

publish_loader() {
    local loader="$1"
    build_loader "$loader"
    local out="$ROOT/artifacts/$loader"
    rm -rf "$out"; mkdir -p "$out"
    find "$ROOT/$loader/build/libs" -maxdepth 1 -name '*.jar' \
        ! -name '*-dev.jar' ! -name '*-sources.jar' -exec cp {} "$out/" \; 2>/dev/null || true
    echo "    artifacts/$loader/ <- $(ls -1 "$out" 2>/dev/null | tr '\n' ' ')"
}

cmd_publish() {
    [ -n "${1:-}" ] || die "publish needs a target: a loader (${LOADERS[*]}) or 'all'"
    CONFIG="Release"
    build_rust
    mkdir -p "$ROOT/artifacts/native"
    cp "$ROOT/rust/target/release/$(native_lib_name)" "$ROOT/artifacts/native/"

    if [ "$1" = "all" ]; then
        local l; for l in "${LOADERS[@]}"; do publish_loader "$l"; done
    else
        require_loader "$1"; publish_loader "$1"
    fi
    echo "==> published (native in artifacts/native/)"
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
    run)            cmd_run "$target" ;;
    test)           cmd_test ;;
    clean)          cmd_clean ;;
    publish)        cmd_publish "$target" ;;
    -h|--help|help) usage ;;
    *) echo "unknown command: $cmd" >&2; usage; exit 2 ;;
esac
