#!/usr/bin/env bash
# release.sh — bump, publish, и обновить ссылки в воркспейсе.
# Использование: ./release.sh <crate-name> --patch|--minor|--major [--dry-run] [--no-publish] [--no-cascade]
#                ./release.sh <crate-name> --publish-only
#
# При --minor/--major все зависимые крейты воркспейса тоже бампятся (минор) и
# публикуются следом в порядке зависимостей — иначе на crates.io остаются
# версии, собранные против старого минора (в 0.x каждый минор несовместим),
# и verify следующего крейта падает на «two different versions of crate».
#
# Примеры:
#   ./release.sh yog-abi --minor          # каскад: abi → gfx → ui → ... → runtime
#   ./release.sh yog-book --patch         # patch: ссылки совместимы, каскада нет
#   ./release.sh yog-api --publish-only   # только публикация текущей версии

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
    echo "  --patch          0.9.3 → 0.9.4  (ссылки не меняются — semver совместимость)"
    echo "  --minor          0.9.3 → 0.10.0 (обновит ссылки и каскадно сбампит зависимые крейты)"
    echo "  --major          0.9.3 → 1.0.0  (аналогично)"
    echo "  --dry-run        только показать, что изменится"
    echo "  --no-publish     сбампить и обновить ссылки без публикации"
    echo "  --no-cascade     не трогать зависимые крейты (как раньше)"
    echo "  --publish-only   только опубликовать текущую версию крейта"
    exit 1
}

CRATE=""
BUMP=""
DRY_RUN=false
NO_PUBLISH=false
PUBLISH_ONLY=false
CASCADE=true

for arg in "$@"; do
    case "$arg" in
        --patch|--minor|--major) BUMP="$arg" ;;
        --dry-run)      DRY_RUN=true ;;
        --no-publish)   NO_PUBLISH=true ;;
        --publish-only) PUBLISH_ONLY=true ;;
        --no-cascade)   CASCADE=false ;;
        --*) die "Неизвестный флаг: $arg"; ;;
        *)
            if [[ -z "$CRATE" ]]; then CRATE="$arg"
            else die "Лишний аргумент: $arg"; fi
            ;;
    esac
done

[[ -z "$CRATE" ]] && { echo "Не указано имя крейта."; usage; }
if ! $PUBLISH_ONLY && [[ -z "$BUMP" ]]; then
    echo "Не указан тип бампа."; usage
fi
# patch-бамп совместим по ссылкам — каскад не нужен
[[ "$BUMP" == "--patch" ]] && CASCADE=false

crate_toml() { echo "$WS/crates/$1/Cargo.toml"; }

crate_version() {
    grep -m1 '^version *= *"' "$(crate_toml "$1")" | sed 's/.*"\([^"]*\)".*/\1/'
}

[[ -f "$(crate_toml "$CRATE")" ]] || die "Крейт не найден: $(crate_toml "$CRATE")"

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

# ── Функция замены версии-ссылки в файле ──────────────────────────────────────
update_ref() {
    local dep="$1" file="$2" old="$3" new="$4"
    # Простая форма:  yog-foo = "0.9"  /  "0.9.2"
    sed -i -E "s|(${dep}[[:space:]]*=[[:space:]]*\")${old}([.\"])|\1${new}\2|g" "$file"
    # Сложная форма:  yog-foo = { version = "0.9", ... }
    sed -i -E "s|(${dep}[[:space:]]*=[[:space:]]*\{[^}]*version[[:space:]]*=[[:space:]]*\")${old}([.\"])|\1${new}\2|g" "$file"
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
    local old_short new_short
    old_short="$(echo "$current" | cut -d. -f1-2)"
    new_short="$maj.$min"

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

    # Ссылки обновляем только когда минор/мажор реально сменился.
    if [[ "$old_short" != "$new_short" ]]; then
        local t
        for t in "$WS/crates"/*/Cargo.toml "$WS/Cargo.toml"; do
            [[ "$t" == "$toml" ]] && continue
            [[ -f "$t" ]] || continue
            if grep -qE "${crate}[[:space:]]*=.*\"${old_short}" "$t" 2>/dev/null; then
                if $DRY_RUN; then
                    dryrun "  ${t#$WS/} : $crate $old_short → $new_short" >&2
                else
                    update_ref "$crate" "$t" "$old_short" "$new_short"
                    ok "  ссылка: ${t#$WS/}" >&2
                fi
            fi
        done
    fi

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
for toml in glob.glob(os.path.join(ws, "crates", "*", "Cargo.toml")):
    text = open(toml).read()
    m = re.search(r'^name *= *"([^"]+)"', text, re.M)
    if not m:
        continue
    name = m.group(1)
    ds = set(re.findall(r'^(yog-[a-z0-9_-]+) *=', text, re.M)) - {name}
    deps[name] = ds
if root not in deps:
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
if $CASCADE; then
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
    if [[ "$c" == "$CRATE" ]]; then
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

# ── Шаг 3: публикация цепочки ─────────────────────────────────────────────────
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
