#!/usr/bin/env python3
"""Checks every source file of the Yog repositories against the automatic
items of CODE_STANDARDS.md and writes the review journal.

    tools/standards_check.py            # all repositories, writes docs/standards-review.tsv
    tools/standards_check.py FILE...    # just these files, findings printed

The journal keeps each file's review status (column `status`) across runs:
a file is `todo` until it has been read by hand against the checklist and
fixed, then `ok`. A file with automatic findings is never `ok`.
"""

import csv
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

LOADER = Path(__file__).resolve().parent.parent
WORKSPACE = LOADER.parent
REPOS = ["Yog-Mod-Loader", "Yog-IDLE", "Substrate-Platform", "HexMod-Yog", "Yog-VLSI", "Yog-Pipes", "Yog-Modlist", "Yog-Argus"]
JOURNAL = LOADER / "docs" / "standards-review.tsv"

SOURCE = re.compile(r"\.(rs|ts|tsx|java)$")
# Not reviewed: generated code, fixtures, test data.
SKIP = re.compile(r"(^|/)(tests/golden|fixtures|generated|target|node_modules)/|\.g\.rs$|\.d\.ts$")

MAX_FILE_LINES = 800
MAX_FN_LINES = 80
MAX_PARAMS = 5

# Words that name a kind, a namespace or a registry: as a string literal they
# are a fact defined somewhere else, repeated.
KIND_WORDS = (
    "item|block|entity|recipe|tag|fluid|biome|potion|effect|attribute|sound|texture|model|"
    "advancement|loot_table|loot_modifier|dimension|font|enchantment|particle|menu"
)
LITERAL_FACT = re.compile(
    r'"(?:(?:' + KIND_WORDS + r')|minecraft|yog|(?:minecraft|yog):[a-z0-9_/.]*'
    r'|[a-z_]+\.(?:yitem|yblock|yentity|ydef|yrecipe|ymodel|yanim|yres|yui|ybook|yfont|ydata)'
    r'|\.(?:yitem|yblock|yentity|ydef|yrecipe|ymodel|yanim|yres|yui|ybook|yfont|ydata|png|json))"'
)
HEX_COLOR = re.compile(r"\b0x[0-9A-Fa-f_]{8,11}\b")
# A fraction other than 0, 1, 2 and 0.5 (arithmetic, not meaning).
FLOAT_MAGIC = re.compile(r"(?<![\w.])(?!(?:[012](?:\.0+)?|0\.50*)(?:f32|f64|f)?\b(?!\.\d))\d+\.\d+(?:f32|f64|f)?\b")
INT_MAGIC = re.compile(r"(?<![\w.#\[])(?!(?:0|1|2|10|100)\b)\d{2,}(?:u8|u16|u32|u64|usize|i32|i64)?\b(?!\.)")
CONST_LINE = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?(const|static)\s|^\s*(export\s+)?const\s+[A-Z_][A-Z0-9_]*\s*[:=]|static final|^\s*((public|private|protected|internal)\s+)?const\s+\w+\s+[A-Z]\w*\s*=")

SILENCE = re.compile(r"#!?\[allow\(|@ts-ignore|@ts-expect-error|eslint-disable|@SuppressWarnings")
PANIC = re.compile(r"\.unwrap\(\)|\.expect\(|\bpanic!\(|\bunreachable!\(|\btodo!\(|\bunimplemented!\(")
PRINT = re.compile(r"\b(println!|eprintln!|dbg!|print!)\(|System\.(out|err)\.print|console\.(log|debug)\(")
TODO = re.compile(r"\b(TODO|FIXME|XXX|HACK)\b")
# A glob import (CODE_STANDARDS: none outside preludes and tests): Rust `use a::*;`, Java
# `import a.*;`. A `pub use a::*` re-export (a facade's surface, like a prelude) isn't one.
GLOB_IMPORT = re.compile(r"^\s*use\s+(?![\w:]*\bprelude::\*)[\w:{}, ]*::\*\s*;|^\s*import\s+(static\s+)?[\w.]+\.\*\s*;")
# A string literal in yog-ids' names — a whole value or a piece of a composed
# one: each is written once in the whole crate (CODE_STANDARDS §0); a repeat
# says the word belongs in `word` (or the group the others compose from).
IDS_NAMES = re.compile(r"(^|/)rust/crates/yog-ids/src/names/")
IDS_LITERAL = re.compile(r'"(?:\\.|[^"\\])+"')


def ids_code(line):
    """The part of a yog-ids names line that isn't a comment."""
    return "" if line.lstrip().startswith("//") else line.split("//")[0]


def ids_literals():
    """Every string literal of yog-ids' names, with how often it's written."""
    counts = {}
    for p in (LOADER / "rust/crates/yog-ids/src/names").glob("*.rs"):
        for line in p.read_text().split("\n"):
            for lit in IDS_LITERAL.findall(ids_code(line)):
                counts[lit] = counts.get(lit, 0) + 1
    return counts


# Sync primitives outside the mod API facade or Substrate-backed engine.
FOREIGN_SYNC = re.compile(r"\bstd::sync\b|\bparking_lot\b|\bonce_cell::|\blazy_static\b")
# Where the sync primitives come from, and the repo that takes no Yog crates.
SYNC_HOME = re.compile(r"(^|/)rust/crates/yog-(sync|runtime|hot-reload|symbols|debugger)/|^Substrate-Platform/")
COMMENTED_CODE = re.compile(r"^\s*//\s*(let |fn |pub |use |if \(|for |return |import |const |\}|[\w.]+\(.*\);\s*$)")


@dataclass
class Findings:
    lines: int = 0
    long_fns: list = field(default_factory=list)
    many_params: list = field(default_factory=list)
    silenced: int = 0
    panics: int = 0
    prints: int = 0
    literal_facts: int = 0
    magic_numbers: int = 0
    todos: int = 0
    commented_code: int = 0
    glob_imports: int = 0
    foreign_sync: int = 0
    ids_repeats: int = 0

    def summary(self):
        out = []
        if self.lines > MAX_FILE_LINES:
            out.append(f"file:{self.lines}")
        if self.long_fns:
            out.append("long-fn:" + ",".join(self.long_fns[:5]) + ("…" if len(self.long_fns) > 5 else ""))
        if self.many_params:
            out.append("params:" + ",".join(self.many_params[:5]) + ("…" if len(self.many_params) > 5 else ""))
        for name in ("silenced", "panics", "prints", "literal_facts", "magic_numbers", "todos", "commented_code", "glob_imports", "foreign_sync", "ids_repeats"):
            n = getattr(self, name)
            if n:
                out.append(f"{name}:{n}")
        return " ".join(out)


# Where the facts are defined (CODE_STANDARDS §0): a literal there is the source, not a repeat.
FACT_HOME = re.compile(r"(^|/)rust/crates/yog-ids/")
# A program's own entry point and its command modules: what they print is its output.
CLI_OUTPUT = re.compile(r"/src/bin/|/src/main\.rs$|/src/cli/|/src/commands/")
# A file that says it's generated: its generator is what's reviewed.
GENERATED = re.compile(r"^\s*(//|/\*|\*).*\b(Generated by|@generated|Don't edit)", re.I)
# A module declared for tests or benches only (`#[cfg(test)] mod x;`).
TEST_CFG = re.compile(r'#\[cfg\((test|any\(test\b[^)]*\)|feature\s*=\s*"(bench|testing)")\)\]')


def test_only_module(path):
    """Whether Rust file `path` is a module its parent declares for tests or benches only."""
    file = WORKSPACE / path
    name = file.stem
    parent_dir = file.parent if name != "mod" else file.parent.parent
    if name == "mod":
        name = file.parent.name
    for parent in (parent_dir / "mod.rs", parent_dir / "lib.rs", parent_dir / "main.rs", parent_dir.with_suffix(".rs")):
        if not parent.is_file():
            continue
        lines = parent.read_text(errors="replace").split("\n")
        for i, l in enumerate(lines):
            if re.match(r"^\s*(pub(\([^)]*\))?\s+)?mod\s+" + re.escape(name) + r"\s*;", l):
                attrs = []
                k = i - 1
                while k >= 0 and lines[k].strip().startswith("#["):
                    attrs.append(lines[k])
                    k -= 1
                if any(TEST_CFG.search(a) for a in attrs):
                    return True
                # Inside a module that is itself for tests or benches only.
                return parent.name not in ("lib.rs", "main.rs") and test_only_module(
                    str(parent.relative_to(WORKSPACE))
                )
    return False


def test_start(lines, path):
    """Index of the first line of the file's test module (Rust), or len."""
    if "/tests/" in path or "/benches/" in path or path.endswith("_test.rs") or path.endswith("tests.rs") or re.search(r"\.(test|bench)\.tsx?$", path):
        return 0
    if path.endswith(".rs") and test_only_module(path):
        return 0
    for i, l in enumerate(lines):
        if re.match(r"^\s*#\[cfg\((test|all\(test\b|any\(test\b)", l):
            # The test module, not a test-only item before it.
            k = i + 1
            while k < len(lines) and lines[k].strip().startswith("#["):
                k += 1
            if k < len(lines) and re.match(r"^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*\{", lines[k]):
                return i
    return len(lines)


# String and character literals: their braces aren't the code's.
LITERALS = re.compile(r'"(?:\\.|[^"\\])*"|b?\'(?:\\.|[^\'\\])\'')
# A line comment (after literals are cut out): its braces aren't code either.
LINE_COMMENT = re.compile(r"//.*")


def parenthesized(sig):
    """What's inside the signature's first parentheses (balanced): its parameters."""
    start = sig.find("(")
    if start < 0:
        return ""
    depth = 0
    for k in range(start, len(sig)):
        depth += sig[k] == "("
        depth -= sig[k] == ")"
        if depth == 0:
            return sig[start + 1 : k]
    return sig[start + 1 :]


def functions(lines, lang):
    """(name, first line, line count, params) of each function body."""
    if lang == "rs":
        start = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?(const\s+)?(async\s+)?(unsafe\s+)?(extern\s+\"[^\"]*\"\s+)?fn\s+(\w+)")
    elif lang == "java":
        start = re.compile(r"^\s*(public|private|protected|static|final|synchronized|\s)+[\w<>\[\],.? ]+\s+(\w+)\s*\(")
    else:
        start = re.compile(r"^\s*(export\s+)?(default\s+)?(async\s+)?function\s+(\w+)|^\s*(export\s+)?const\s+(\w+)\s*=\s*(async\s*)?\(")
    out = []
    i = 0
    while i < len(lines):
        m = start.match(lines[i])
        if not m:
            i += 1
            continue
        name = [g for g in m.groups() if g and re.fullmatch(r"\w+", g) and g not in ("pub", "const", "async", "unsafe", "export", "default", "public", "private", "protected", "static", "final", "synchronized")]
        name = name[-1] if name else "?"
        # The signature runs to the body's `{` (or `;` for a declaration).
        sig, j = "", i
        while j < len(lines) and "{" not in LITERALS.sub("", lines[j]) and not lines[j].rstrip().endswith(";"):
            sig += lines[j]
            j += 1
        if j >= len(lines) or "{" not in LITERALS.sub("", lines[j]):
            i = j + 1
            continue
        sig += lines[j].split("{")[0]
        params = parenthesized(sig)
        depth, n = 0, 0
        params_n = 0
        if params.strip():
            d = 0
            params_n = 1
            for c in params:
                d += c in "<([{"
                d -= c in ">)]}"
                if c == "," and d == 0:
                    params_n += 1
            if params.rstrip().endswith(","):
                params_n -= 1
            if re.search(r"&\s*(mut\s+)?self\b|\bself\b", params.split(",")[0]):
                params_n -= 1
        k = j
        while k < len(lines):
            code = LINE_COMMENT.sub("", LITERALS.sub("", lines[k]))
            depth += code.count("{") - code.count("}")
            if depth <= 0 and k >= j:
                break
            k += 1
        out.append((name, i + 1, k - i + 1, params_n))
        i = j + 1
    return out


# A Java class whose static methods native code calls (`jcall!`): their
# signatures are the JNI bridge's, an ABI like Rust's `extern` fns.
JNI_SURFACE = re.compile(r"/Native\w*\.java$")


def fixed_signature(lines, i, path):
    """Whether the function at line `i` can't choose its parameters: an FFI
    entry (`extern`, the JNI surface), a Java override (the overridden
    method's), a bus service's host (`@YogServiceHost`: the request's
    components, as the generated code spreads them), a record's components
    (a data type, not a function)."""
    if "extern" in lines[i] or JNI_SURFACE.search(path) or re.search(r"\brecord\s+\w+\s*\(", lines[i]):
        return True
    k = i - 1
    while k >= 0 and lines[k].strip().startswith(("@", "//", "/*", "*")):
        if lines[k].strip().startswith(("@Override", "@YogServiceHost")):
            return True
        k -= 1
    return False


def const_fn_lines(lines):
    """Lines inside Rust `const fn`s: a panic there is a compile error, not a runtime one."""
    out, depth, inside = set(), 0, False
    for i, l in enumerate(lines):
        if not inside and re.search(r"\bconst\s+fn\b", l):
            inside, depth = True, 0
        if inside:
            out.add(i)
            depth += l.count("{") - l.count("}")
            if depth <= 0 and "}" in l:
                inside = False
    return out


def const_item_lines(lines):
    """Lines of a `const`/`static` item, all of it to its `;` — rustfmt splits
    a long one over lines, and a number in it is its definition."""
    out, depth, inside = set(), 0, False
    for i, l in enumerate(lines):
        if not inside and CONST_LINE.search(l) and not re.search(r"\bconst\s+fn\b", l):
            inside, depth = True, 0
        if inside:
            out.add(i)
            code = LITERALS.sub('""', l.split("//")[0])
            depth += sum(code.count(c) for c in "([{") - sum(code.count(c) for c in ")]}")
            if depth <= 0 and code.rstrip().endswith(";"):
                inside = False
    return out


def check(path, text):
    lang = path.rsplit(".", 1)[1]
    lang = "ts" if lang in ("ts", "tsx") else lang
    lines = text.split("\n")
    if any(GENERATED.match(l) for l in lines[:6]):
        return Findings()
    f = Findings(lines=len(lines))
    tests = test_start(lines, path)
    for name, line, length, params in functions(lines[:tests], lang):
        if length > MAX_FN_LINES:
            f.long_fns.append(f"{name}@{line}({length})")
        if params > MAX_PARAMS and not fixed_signature(lines, line - 1, path):
            f.many_params.append(f"{name}@{line}({params})")
    in_const_fn = const_fn_lines(lines) if lang == "rs" else set()
    in_const = const_item_lines(lines)
    if IDS_NAMES.search(path):
        repeated = ids_literals()
        f.ids_repeats = sum(1 for l in lines for lit in IDS_LITERAL.findall(ids_code(l)) if repeated[lit] > 1)
    for i, l in enumerate(lines[:tests]):
        code = l.split("//")[0] if lang != "java" or "//" in l else l
        # A block comment's line (`/** … */`, ` * …`) says, it isn't code.
        if l.lstrip().startswith(("/*", "* ", "*/")) or l.strip() == "*":
            code = ""
        # What a string says isn't code (a macro's description names `panic!`,
        # a Java generator's constant is `@SuppressWarnings`'s text).
        bare = LITERALS.sub('""', code)
        if SILENCE.search(bare) and "//" not in l and (i == 0 or "//" not in lines[i - 1]):
            f.silenced += 1
        if lang == "rs" and PANIC.search(bare) and i not in in_const_fn:
            f.panics += 1
        # A command line tool's output is what it's for (CODE_STANDARDS: CLI output is exempt).
        if PRINT.search(bare) and not CLI_OUTPUT.search(path):
            f.prints += 1
        if TODO.search(l):
            f.todos += 1
        if COMMENTED_CODE.match(l):
            f.commented_code += 1
        if GLOB_IMPORT.match(l):
            f.glob_imports += 1
        if lang == "rs" and not SYNC_HOME.search(path) and FOREIGN_SYNC.search(LITERALS.sub('""', code)):
            f.foreign_sync += 1
        if i in in_const or l.strip().startswith(("///", "//", "*", "/*")):
            continue
        # Where the facts are defined, a literal is the definition, not a repeat.
        if not FACT_HOME.search(path):
            f.literal_facts += len(LITERAL_FACT.findall(code))
            f.magic_numbers += len(HEX_COLOR.findall(code)) + len(FLOAT_MAGIC.findall(code))
    return f


def tracked_files():
    for repo in REPOS:
        root = WORKSPACE / repo
        files = subprocess.run(["git", "-C", str(root), "ls-files"], capture_output=True, text=True).stdout.split("\n")
        for rel in files:
            if SOURCE.search(rel) and not SKIP.search(rel):
                yield f"{repo}/{rel}"


def read_journal():
    if not JOURNAL.exists():
        return {}
    with open(JOURNAL, newline="") as fh:
        return {r["path"]: r for r in csv.DictReader(fh, delimiter="\t")}


def main(argv):
    if argv:
        for p in argv:
            rel = os.path.relpath(os.path.abspath(p), WORKSPACE)
            print(f"{rel}\t{check(rel, Path(p).read_text(errors='replace')).summary()}")
        return 0
    old = read_journal()
    rows = []
    for rel in tracked_files():
        text = (WORKSPACE / rel).read_text(errors="replace")
        summary = check(rel, text).summary()
        status = old.get(rel, {}).get("status", "todo")
        if summary and status == "ok":
            status = "todo"
        rows.append({"path": rel, "findings": summary, "status": status})
    JOURNAL.parent.mkdir(parents=True, exist_ok=True)
    with open(JOURNAL, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["path", "findings", "status"], delimiter="\t")
        w.writeheader()
        w.writerows(rows)
    total = len(rows)
    clean = sum(1 for r in rows if not r["findings"])
    ok = sum(1 for r in rows if r["status"] == "ok")
    print(f"{total} files: {total - clean} with automatic findings, {ok} reviewed ok → {JOURNAL.relative_to(LOADER)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
