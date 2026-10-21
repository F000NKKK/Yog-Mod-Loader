#!/usr/bin/env bash
# release.sh — bump, publish, и обновить ссылки (корневой [workspace.dependencies] и Yog-IDLE).
# Использование: ./release.sh <crate-name> --patch|--minor|--major [--dry-run] [--no-publish] [--no-cascade]
#                ./release.sh <crate-name> --publish-only
#                ./release.sh --all --minor|--major [--dry-run] [--no-publish]
#
# При --minor/--major все зависимые крейты воркспейса тоже бампятся (минор) и
# публикуются следом в порядке зависимостей — иначе на crates.io остаются
# версии, собранные против старого минора (в 0.x каждый минор несовместим),
# и verify следующего крейта падает на «two different versions of crate».
#
# Примеры:
#   ./release.sh yog-abi --minor          # каскад: abi → gfx → ui → ... → runtime
#   ./release.sh yog-book --patch         # patch: только ссылки, каскада нет
#   ./release.sh yog-api --publish-only   # только публикация текущей версии
#   ./release.sh --all --minor            # все публикуемые крейты, каждый один раз

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$SCRIPT_DIR"

# ── Цвета ─────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[release]${RESET} $*"; }
ok()      { echo -e "${GREEN}[ok]${RESET} $*"; }
warn()    { echo -e "${YELLOW}[warn]${RESET} $*"; }
die()     { echo -e "${RED}[error]${RESET} $*" >&2; exit 1; }
dryrun()  { echo -e "${YELLOW}[dry-run]${RESET} $*"; }

# ── Аргументы ─────────────────────────────────────────────────────────────────
usage() {
    echo -e "${BOLD}Использование:${RESET} $0 <crate-name> --patch|--minor|--major [--dry-run] [--no-publish] [--no-cascade]"
    echo ""
    echo "  --patch          0.9.3 → 0.9.4  (ссылки пинятся новой версией, каскада нет)"
    echo "  --minor          0.9.3 → 0.10.0 (обновит ссылки и каскадно сбампит зависимые крейты)"
    echo "  --major          0.9.3 → 1.0.0  (аналогично)"
    echo "  --dry-run        только показать, что изменится"
    echo "  --no-publish     сбампить и обновить ссылки без публикации"
    echo "  --no-cascade     не трогать зависимые крейты (как раньше)"
    echo "  --publish-only   только опубликовать текущую версию крейта"
    echo "  --all            вместо одного крейта — все публикуемые крейты воркспейса"
    exit 1
}

CRATE=""
BUMP=""
DRY_RUN=false
NO_PUBLISH=false
PUBLISH_ONLY=false
CASCADE=true
ALL=false

for arg in "$@"; do
    case "$arg" in
        --patch|--minor|--major) BUMP="$arg" ;;
        --dry-run)      DRY_RUN=true ;;
        --no-publish)   NO_PUBLISH=true ;;
        --publish-only) PUBLISH_ONLY=true ;;
        --no-cascade)   CASCADE=false ;;
        --all)          ALL=true ;;
        --*) die "Неизвестный флаг: $arg"; ;;
        *)
            if [[ -z "$CRATE" ]]; then CRATE="$arg"
            else die "Лишний аргумент: $arg"; fi
            ;;
    esac
done

if $ALL; then
    [[ -n "$CRATE" ]] && die "--all не принимает имя крейта"
    $PUBLISH_ONLY && die "--all не сочетается с --publish-only"
    [[ "$BUMP" == "--patch" ]] && die "--all бампит минор или мажор: изменились зависимости"
    CRATE="--all"
fi
[[ -z "$CRATE" ]] && { echo "Не указано имя крейта."; usage; }
if ! $PUBLISH_ONLY && [[ -z "$BUMP" ]]; then
    echo "Не указан тип бампа."; usage
fi
# patch-бамп совместим — зависимые пересобирать не нужно
[[ "$BUMP" == "--patch" ]] && CASCADE=false

crate_toml() { echo "$WS/crates/$1/Cargo.toml"; }

crate_version() {
    grep -m1 '^version *= *"' "$(crate_toml "$1")" | sed 's/.*"\([^"]*\)".*/\1/'
}

$ALL || [[ -f "$(crate_toml "$CRATE")" ]] || die "Крейт не найден: $(crate_toml "$CRATE")"

# ── Публикация с ретраями ─────────────────────────────────────────────────────
# Без фиксированного ожидания индексации: cargo сам ждёт появления крейта в
# индексе после аплоада, а если следующий publish всё же не резолвит только
# что изданную зависимость — ретраим: 1 попытка + 3 ретрая раз в 5 сек.
publish_with_retry() {
    local crate="$1" ver="$2"
    local attempt=0 max=4 out rc
    while true; do
        attempt=$((attempt + 1))
        info "Публикую $crate v$ver на crates.io (попытка $attempt/$max)..."
        set +e
        out=$(cargo publish -p "$crate" 2>&1); rc=$?
        set -e
        if [[ $rc -eq 0 ]]; then
            ok "Опубликовано: $crate v$ver"
            return 0
        fi
        if echo "$out" | grep -qiE "already uploaded|already exists on crates.io"; then
            warn "$crate v$ver уже на crates.io — пропускаю"
            return 0
        fi
        if [[ $attempt -ge $max ]]; then
            echo "$out" | tail -40
            die "Публикация $crate v$ver не удалась после $max попыток"
        fi
        echo "$out" | tail -5
        warn "Не удалось (rc=$rc) — ретрай через 5 сек..."
        sleep 5
    done
}

# ── --publish-only: публикуем как есть, без бампа ─────────────────────────────
if $PUBLISH_ONLY; then
    CURRENT="$(crate_version "$CRATE")"
    echo ""
    echo -e "${BOLD}Крейт:${RESET}   $CRATE"
    echo -e "${BOLD}Версия:${RESET}  ${GREEN}$CURRENT${RESET} (без изменений)"
    echo -e "${BOLD}Режим:${RESET}   --publish-only"
    echo ""
    info "cargo check -p $CRATE ..."
    cargo check --offline -p "$CRATE" 2>&1 | tail -3
    ok "check пройден"
    echo ""
    if $DRY_RUN; then
        dryrun "cargo publish -p $CRATE"
    elif $NO_PUBLISH; then
        warn "--no-publish: пропускаю cargo publish"
    else
        publish_with_retry "$CRATE" "$CURRENT"
    fi
    echo ""
    echo -e "${GREEN}${BOLD}Готово!${RESET} $CRATE v$CURRENT"
    echo ""
    exit 0
fi

# ── Ссылки на версию крейта ───────────────────────────────────────────────────
# Версии внутренних крейтов живут в одном месте — корневом
# [workspace.dependencies] (крейты берут их через `{ workspace = true }`).
# Требования пинятся полной версией, поэтому обновляются при любом бампе.
# Вне воркспейса на крейты ссылается только Yog-IDLE (через [patch] на пути).
REF_FILES=("$WS/Cargo.toml" "$WS/../../Yog-IDLE/src-tauri/Cargo.toml")

# set_ref <crate> <file> <new>: `crate = "x"` и `crate = { version = "x", ... }`.
set_ref() {
    local dep="$1" file="$2" new="$3"
    sed -i -E \
        -e "s|^(${dep}[[:space:]]*=[[:space:]]*\")[^\"]*(\")|\1${new}\2|" \
        -e "s|^(${dep}[[:space:]]*=[[:space:]]*\{[^}]*version[[:space:]]*=[[:space:]]*\")[^\"]*(\")|\1${new}\2|" \
        "$file"
}

# ── Бамп одного крейта + обновление ссылок на него по воркспейсу ──────────────
# bump_crate <crate> <--patch|--minor|--major>; печатает новую версию в stdout.
bump_crate() {
    local crate="$1" bump="$2"
    local toml; toml="$(crate_toml "$crate")"
    local current; current="$(crate_version "$crate")"
    [[ -n "$current" ]] || die "Не удалось прочитать version из $toml"

    local maj min pat
    IFS='.' read -r maj min pat <<< "$current"
    case "$bump" in
        --major) maj=$((maj + 1)); min=0; pat=0 ;;
        --minor) min=$((min + 1)); pat=0 ;;
        --patch) pat=$((pat + 1)) ;;
    esac
    local new_version="$maj.$min.$pat"

    # Статусы — в stderr: stdout этой функции захватывается как результат.
    if $DRY_RUN; then
        dryrun "$crate: $current → $new_version" >&2
    else
        awk -v old="$current" -v new="$new_version" '
            /^\[/{in_pkg=0}
            /^\[package\]/{in_pkg=1}
            in_pkg && /^version *=/ && !done {
                sub(old, new); done=1
            }
            {print}
        ' "$toml" > "$toml.tmp" && mv "$toml.tmp" "$toml"
        ok "$crate: $current → $new_version" >&2
    fi

    local t
    for t in "${REF_FILES[@]}"; do
        [[ -f "$t" ]] || continue
        grep -qE "^${crate}[[:space:]]*=.*\"" "$t" || continue
        if $DRY_RUN; then
            dryrun "  $(realpath --relative-to="$WS" "$t") : $crate → $new_version" >&2
        else
            set_ref "$crate" "$t" "$new_version"
            ok "  ссылка: $(realpath --relative-to="$WS" "$t")" >&2
        fi
    done

    echo "$new_version"
}

# ── Каскад: зависимые крейты в порядке публикации ─────────────────────────────
# Печатает "<crate> <crate> ..." — сам корень + все транзитивные зависимые,
# топологически (зависимости раньше зависящих).
cascade_order() {
    python3 - "$WS" "$CRATE" <<'PY'
import sys, re, glob, os
ws, root = sys.argv[1], sys.argv[2]
deps = {}
publishable = {}
for toml in glob.glob(os.path.join(ws, "crates", "*", "Cargo.toml")):
    text = open(toml).read()
    m = re.search(r'^name *= *"([^"]+)"', text, re.M)
    if not m:
        continue
    name = m.group(1)
    ds = set(re.findall(r'^(yog-[a-z0-9_-]+) *=', text, re.M)) - {name}
    deps[name] = ds
    publishable[name] = not re.search(r'^publish *= *false', text, re.M)
if root not in deps and root != "--all":
    sys.exit(f"crate {root} not found in workspace")
# Транзитивные зависимые от root
dependents = set()
changed = True
while changed:
    changed = False
    for c, ds in deps.items():
        if c == root or c in dependents:
            continue
        if root in ds or (ds & dependents):
            dependents.add(c)
            changed = True
# Топологический порядок внутри {root} ∪ dependents
sel = dependents | {root}
if root == "--all":
    sel = {c for c, t in publishable.items() if t}
order, placed = [], set()
while len(order) < len(sel):
    progressed = False
    for c in sorted(sel - placed):
        if (deps[c] & sel) <= placed:
            order.append(c)
            placed.add(c)
            progressed = True
            break
    if not progressed:  # цикл в графе — публикуем как есть
        order.extend(sorted(sel - placed))
        break
print(" ".join(order))
PY
}

# ── Составляем план ───────────────────────────────────────────────────────────
if $CASCADE || $ALL; then
    PLAN="$(cascade_order)" || die "не удалось построить каскад"
else
    PLAN="$CRATE"
fi

echo ""
echo -e "${BOLD}Крейт:${RESET}    $CRATE ($BUMP)"
if [[ "$PLAN" != "$CRATE" ]]; then
    echo -e "${BOLD}Каскад:${RESET}   $PLAN"
    echo -e "          (зависимые бампятся минором и публикуются следом)"
fi
echo ""

# ── Шаг 1: бампы + обновление ссылок, в порядке зависимостей ──────────────────
declare -A NEW_VERSIONS
for c in $PLAN; do
    if [[ "$c" == "$CRATE" ]] || $ALL; then
        NEW_VERSIONS[$c]="$(bump_crate "$c" "$BUMP" | tail -1)"
    else
        NEW_VERSIONS[$c]="$(bump_crate "$c" --minor | tail -1)"
    fi
done

# ── Шаг 2: один общий check по воркспейсу ────────────────────────────────────
echo ""
info "cargo check --workspace ..."
if $DRY_RUN; then
    dryrun "cargo check --workspace (пропущено)"
else
    cargo check --offline --workspace 2>&1 | tail -3
    ok "check пройден"
fi

# ── Шаг 3: коммит бампов ──────────────────────────────────────────────────────
# cargo publish не берёт незакоммиченное: версии и ссылки уходят коммитом.
if ! $DRY_RUN && ! $NO_PUBLISH; then
    git -C "$WS" add Cargo.toml Cargo.lock crates/*/Cargo.toml
    git -C "$WS" commit -q -m "chore(release): $(for c in $PLAN; do printf '%s %s, ' "$c" "${NEW_VERSIONS[$c]}"; done | sed 's/, $//')"
    ok "бампы закоммичены"
fi

# ── Шаг 4: публикация цепочки ─────────────────────────────────────────────────
echo ""
if $NO_PUBLISH; then
    warn "--no-publish: пропускаю cargo publish"
elif $DRY_RUN; then
    for c in $PLAN; do dryrun "cargo publish -p $c   # v${NEW_VERSIONS[$c]}"; done
else
    for c in $PLAN; do
        publish_with_retry "$c" "${NEW_VERSIONS[$c]}"
    done
fi

# ── Итог ──────────────────────────────────────────────────────────────────────
echo ""
if $DRY_RUN; then
    echo -e "${YELLOW}[dry-run]${RESET} Ничего не изменено. Убери --dry-run для реального запуска."
else
    echo -e "${GREEN}${BOLD}Готово!${RESET}"
    for c in $PLAN; do
        echo -e "  $c → ${NEW_VERSIONS[$c]}  https://crates.io/crates/$c/${NEW_VERSIONS[$c]}"
    done
fi
echo ""
