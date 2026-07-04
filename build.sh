#!/usr/bin/env bash
# Yog build — a dotnet-style task runner.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Implemented loaders. Add 'neoforge' / 'forge' here when their hosts land.
LOADERS=(fabric neoforge)

# The active MC platform for each loader is set by minecraft_version inside
# <loader>/gradle.properties; build.sh does not need to know it separately.
# Version-specific Mixin sources live under <loader>/platforms/<mc_version>/.

CONFIG="Release"   # Debug | Release
RUN_CLIENT=0
NO_BUILD=0

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
      --no-build                        run: skip build, just launch client/server
  -h, --help

Examples:
  ./build.sh build
  ./build.sh run fabric --client
  ./build.sh run fabric --client --no-build
  ./build.sh publish fabric
  ./build.sh publish all
  ./build.sh test -c Debug
  ./build.sh clean
EOF
}

die() { echo "error: $*" >&2; exit 1; }

# ── toolchain helpers ────────────────────────────────────────────────────────

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
            forge) die "'$1' is not implemented yet (roadmap)" ;;
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

# ── build steps ──────────────────────────────────────────────────────────────

cargo_build() {
    local flag=""; [ "$CONFIG" = "Release" ] && flag="--release"
    cargo build $flag --manifest-path "$ROOT/rust/Cargo.toml"
}

# Supported runtime targets: "triple|tag|os".
RUNTIME_TARGETS=(
    "x86_64-unknown-linux-gnu|linux-x86_64|linux"
    "aarch64-unknown-linux-gnu|linux-aarch64|linux"
    "x86_64-pc-windows-gnu|windows-x86_64|windows"
    "x86_64-apple-darwin|macos-x86_64|macos"
    "aarch64-apple-darwin|macos-aarch64|macos"
)

runtime_lib_for_os() {
    case "$1" in
        windows) echo "yog_runtime.dll" ;;
        macos)   echo "libyog_runtime.dylib" ;;
        *)       echo "libyog_runtime.so" ;;
    esac
}

cargo_builder() { command -v cargo-zigbuild >/dev/null 2>&1 && echo zigbuild || echo build; }

# Build the runtime for every available platform and embed each into the loader
# jars (resources/natives/<tag>/) so a single jar works on all OSes.
build_rust() {
    echo "==> build rust ($CONFIG) — all platforms"
    local builder profile installed built l
    builder="$(cargo_builder)"
    profile=""; [ "$CONFIG" = "Release" ] && profile="--release"
    installed="$(rustup target list --installed 2>/dev/null)"
    built=()
    for spec in "${RUNTIME_TARGETS[@]}"; do
        IFS='|' read -r triple tag os <<<"$spec"
        if ! grep -qx "$triple" <<<"$installed"; then
            echo "    skip $tag (target not installed)"
            continue
        fi
        if cargo "$builder" $profile --target "$triple" -p yog-runtime \
                --manifest-path "$ROOT/rust/Cargo.toml" >/dev/null 2>&1; then
            local src="$ROOT/rust/target/$triple/$(cargo_profile_dir)/$(runtime_lib_for_os "$os")"
            if [ -f "$src" ]; then
                for l in "${LOADERS[@]}"; do
                    mkdir -p "$ROOT/$l/src/main/resources/natives/$tag"
                    cp "$src" "$ROOT/$l/src/main/resources/natives/$tag/"
                done
                built+=("$tag")
            fi
        else
            echo "    skip $tag (build failed — toolchain/SDK?)"
        fi
    done
    [ ${#built[@]} -gt 0 ] || die "no runtime platform built"
    echo "    embedded runtime for: ${built[*]}"
}

# Build the example mod into a .yog and stage it in each loader's dev mods dir.
build_example() {
    echo "==> build example-mod (.yog)"
    cargo build --release -p yog-cli --manifest-path "$ROOT/rust/Cargo.toml"
    ( cd "$ROOT/example-mod" && "$ROOT/rust/target/release/yog" build )
    local l
    for l in "${LOADERS[@]}"; do
        mkdir -p "$ROOT/$l/run/yog-mods"
        rm -f "$ROOT/$l/run/yog-mods/"*.yog
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
    if [ "$NO_BUILD" -eq 0 ]; then
        build_rust
        build_example
    else
        echo "==> run: $loader — skipping build (--no-build)"
    fi
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
    build_rust      # embeds all-platform natives into the loader jars
    build_example   # the example .yog mod

    if [ "$1" = "all" ]; then
        local l; for l in "${LOADERS[@]}"; do publish_loader "$l"; done
    else
        require_loader "$1"; publish_loader "$1"
    fi

    mkdir -p "$ROOT/artifacts/mods"
    cp "$ROOT/example-mod/artifacts/"*.yog "$ROOT/artifacts/mods/" 2>/dev/null || true
    echo "==> published to artifacts/ (self-contained jars + .yog mods)"
}

# ── dispatch ─────────────────────────────────────────────────────────────────

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
        --no-build) NO_BUILD=1; shift ;;
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