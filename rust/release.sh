#!/usr/bin/env bash
# release.sh — bump, publish, и обновить ссылки в воркспейсе.
# Использование: ./release.sh <crate-name> --patch|--minor|--major [--dry-run] [--no-publish]
#
# Примеры:
#   ./release.sh yog-api --minor
#   ./release.sh yog-book --patch --dry-run

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
    echo -e "${BOLD}Использование:${RESET} $0 <crate-name> --patch|--minor|--major [--dry-run] [--no-publish]"
    echo ""
    echo "  --patch        0.9.3 → 0.9.4  (ссылки не меняются — semver совместимость)"
    echo "  --minor        0.9.3 → 0.10.0 (обновит все ссылки «0.9» → «0.10»)"
    echo "  --major        0.9.3 → 1.0.0  (обновит все ссылки «0.9» → «1.0»)"
    echo "  --dry-run      только показать, что изменится"
    echo "  --no-publish   сбампить и обновить ссылки без публикации"
    exit 1
}

CRATE=""
BUMP=""
DRY_RUN=false
NO_PUBLISH=false

for arg in "$@"; do
    case "$arg" in
        --patch|--minor|--major) BUMP="$arg" ;;
        --dry-run)    DRY_RUN=true ;;
        --no-publish) NO_PUBLISH=true ;;
        --*) die "Неизвестный флаг: $arg"; ;;
        *)
            if [[ -z "$CRATE" ]]; then CRATE="$arg"
            else die "Лишний аргумент: $arg"; fi
            ;;
    esac
done

[[ -z "$CRATE" ]] && { echo "Не указано имя крейта."; usage; }
[[ -z "$BUMP"  ]] && { echo "Не указан тип бампа.";   usage; }

# ── Путь к крейту ─────────────────────────────────────────────────────────────
CRATE_DIR="$WS/crates/$CRATE"
CRATE_TOML="$CRATE_DIR/Cargo.toml"

[[ -f "$CRATE_TOML" ]] || die "Крейт не найден: $CRATE_TOML"

# ── Читаем текущую версию ─────────────────────────────────────────────────────
# Ищем строку version = "X.Y.Z" в секции [package].
# Используем grep+sed (портативно; трёхаргументный match — только gawk).
CURRENT=$(grep -m1 '^version *= *"' "$CRATE_TOML" | sed 's/.*"\([^"]*\)".*/\1/')

[[ -z "$CURRENT" ]] && die "Не удалось прочитать version из $CRATE_TOML"

if [[ "$CURRENT" == *"workspace"* ]] || [[ "$CURRENT" == *"true"* ]]; then
    die "$CRATE использует version.workspace — замени на явную версию (sed -i 's/version.workspace = true/version = \"X.Y.Z\"/' Cargo.toml)"
fi

# ── Вычисляем новую версию ────────────────────────────────────────────────────
IFS='.' read -r VER_MAJOR VER_MINOR VER_PATCH <<< "$CURRENT"

case "$BUMP" in
    --major) VER_MAJOR=$((VER_MAJOR + 1)); VER_MINOR=0; VER_PATCH=0 ;;
    --minor) VER_MINOR=$((VER_MINOR + 1)); VER_PATCH=0 ;;
    --patch) VER_PATCH=$((VER_PATCH + 1)) ;;
esac

NEW_VERSION="$VER_MAJOR.$VER_MINOR.$VER_PATCH"

# Двузначные формы для поиска в зависимостях
OLD_SHORT="$( echo "$CURRENT" | cut -d. -f1-2 )"   # "0.9"
NEW_SHORT="$VER_MAJOR.$VER_MINOR"                   # "0.10"

echo ""
echo -e "${BOLD}Крейт:${RESET}   $CRATE"
echo -e "${BOLD}Версия:${RESET}  $CURRENT → ${GREEN}$NEW_VERSION${RESET}"
echo -e "${BOLD}Бамп:${RESET}    $BUMP"

# ── Функция замены версии в файле ─────────────────────────────────────────────
update_ref() {
    local file="$1"
    local old="$2"
    local new="$3"

    # Паттерны, которые нужно обновить:
    #   yog-foo = "0.9"
    #   yog-foo = { version = "0.9", ... }
    #   yog-foo = { version = "0.9.x", ... }   (трёхзначные тоже)

    if $DRY_RUN; then
        if grep -qE "$CRATE[[:space:]]*=[[:space:]]*\"$old" "$file" \
        || grep -qE "$CRATE[[:space:]]*=[[:space:]]*\{[^}]*version[[:space:]]*=[[:space:]]*\"$old" "$file"; then
            dryrun "$(basename "$(dirname "$file")")/Cargo.toml : $old → $new"
        fi
        return
    fi

    # Замена: простая форма  yog-foo = "0.9"  или  yog-foo = "0.9.2"
    sed -i -E "s|($CRATE[[:space:]]*=[[:space:]]*\")${old}([.\"])|\1${new}\2|g" "$file"
    # Замена: сложная форма  yog-foo = { version = "0.9", ... }
    sed -i -E "s|($CRATE[[:space:]]*=[[:space:]]*\{[^}]*version[[:space:]]*=[[:space:]]*\")${old}([.\"])|\1${new}\2|g" "$file"
}

# ── Шаг 1: бампить версию в самом крейте ──────────────────────────────────────
echo ""
info "Обновляю версию в $CRATE_TOML"
if $DRY_RUN; then
    dryrun "$CRATE_TOML : version = \"$CURRENT\" → \"$NEW_VERSION\""
else
    # Меняем только в секции [package], первое вхождение version
    awk -v old="$CURRENT" -v new="$NEW_VERSION" '
        /^\[/{in_pkg=0}
        /^\[package\]/{in_pkg=1}
        in_pkg && /^version *=/ && !done {
            sub(old, new); done=1
        }
        {print}
    ' "$CRATE_TOML" > "$CRATE_TOML.tmp" && mv "$CRATE_TOML.tmp" "$CRATE_TOML"
    ok "Версия обновлена: $CURRENT → $NEW_VERSION"
fi

# ── Шаг 2: cargo check перед публикацией ──────────────────────────────────────
echo ""
info "cargo check -p $CRATE ..."
if $DRY_RUN; then
    dryrun "cargo check -p $CRATE (пропущено)"
else
    cargo check --offline -p "$CRATE" 2>&1 | tail -5
    ok "check пройден"
fi

# ── Шаг 3: публикация ─────────────────────────────────────────────────────────
echo ""
if $NO_PUBLISH; then
    warn "--no-publish: пропускаю cargo publish"
elif $DRY_RUN; then
    dryrun "cargo publish -p $CRATE"
else
    info "Публикую $CRATE v$NEW_VERSION на crates.io..."
    cargo publish -p "$CRATE"
    ok "Опубликовано!"

    info "Жду 45 сек — crates.io индексирует пакет..."
    for i in $(seq 45 -1 1); do
        printf "\r  %2d сек..." "$i"
        sleep 1
    done
    echo ""
    ok "Готово, продолжаем"
fi

# ── Шаг 4: обновить ссылки в воркспейсе ──────────────────────────────────────
echo ""
info "Обновляю ссылки во всех крейтах воркспейса..."

# Patch-бамп: старый short == новый short ("0.9" → "0.9"), ссылки совместимы — ничего менять
if [[ "$OLD_SHORT" == "$NEW_SHORT" ]]; then
    warn "patch-бамп: ссылки «$OLD_SHORT» остаются совместимыми, обновление не нужно"
else
    UPDATED_COUNT=0
    for toml in "$WS/crates"/*/Cargo.toml "$WS/Cargo.toml"; do
        [[ "$toml" == "$CRATE_TOML" ]] && continue
        [[ -f "$toml" ]] || continue

        if grep -qE "$CRATE[[:space:]]*=.*\"$OLD_SHORT" "$toml" 2>/dev/null; then
            update_ref "$toml" "$OLD_SHORT" "$NEW_SHORT"
            if ! $DRY_RUN; then
                ok "Обновлено: ${toml#$WS/}"
            fi
            UPDATED_COUNT=$((UPDATED_COUNT + 1))
        fi
    done

    if [[ $UPDATED_COUNT -eq 0 ]]; then
        warn "Ни один крейт не ссылается на $CRATE = \"$OLD_SHORT\""
    elif ! $DRY_RUN; then
        ok "Обновлено крейтов: $UPDATED_COUNT"
    fi
fi

# ── Итог ──────────────────────────────────────────────────────────────────────
echo ""
if $DRY_RUN; then
    echo -e "${YELLOW}[dry-run]${RESET} Ничего не изменено. Убери --dry-run для реального запуска."
else
    echo -e "${GREEN}${BOLD}Готово!${RESET} $CRATE $CURRENT → $NEW_VERSION"
    if ! $NO_PUBLISH; then
        echo -e "  crates.io: https://crates.io/crates/$CRATE/$NEW_VERSION"
    fi
fi
echo ""
