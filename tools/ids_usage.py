#!/usr/bin/env python3
"""Who uses each `yog-ids` name — a review aid for CODE_STANDARDS §0 ("where a
constant lives"), not a verdict: it reads Rust use-trees, Java's
`YogNames.Group.NAME` and TS's generated objects by text, so a name reached
another way (a macro, a whole group iterated) can look unused. Delete or move
a name only after the compiler and a search of the other side agree.

    tools/ids_usage.py            # summary: unused, one-crate, groups of one crate
    tools/ids_usage.py GROUP...   # each name of these groups and its users
"""

import collections
import re
import sys
from pathlib import Path

LOADER = Path(__file__).resolve().parent.parent
WORKSPACE = LOADER.parent
NAMES = LOADER / "rust/crates/yog-ids/src/names"
RUST = ["Yog-Mod-Loader/rust/crates", "Yog-IDLE/src-tauri/src", "Yog-Installer/crates",
        "Yog-Pipes", "Yog-VLSI", "HexMod-Yog", "Yog-Modlist"]
JAVA = ["Yog-Mod-Loader/java-common", "Yog-Mod-Loader/fabric", "Yog-Mod-Loader/forge", "Yog-Mod-Loader/neoforge"]
TS = ["Yog-IDLE/src"]
SKIP = ("/target/", "/build/", "node_modules", ".yog-build", "generated", "YogNames.java")
GROUP = re.compile(r"^    (\w+) \{\s*$")
NAME = re.compile(r"^        ([A-Z][A-Z0-9_]*) = (.+?),?\s*$")


def names():
    """(group, NAME, value) of every names! constant."""
    out = []
    for p in sorted(NAMES.glob("*.rs")):
        group = None
        for line in p.read_text().split("\n"):
            if m := GROUP.match(line):
                group = m.group(1)
            elif (m := NAME.match(line)) and group:
                out.append((group, m.group(1), m.group(2)))
    return out


def sources(roots, ext):
    for root in roots:
        for p in (WORKSPACE / root).rglob(f"*.{ext}"):
            s = str(p)
            if not any(k in s for k in SKIP):
                yield s, p.read_text(errors="replace")


def owner(path):
    m = re.search(r"crates/([\w-]+)/", path) or re.search(r"Projects/yog/([\w-]+)/", path)
    return m.group(1) if m else path


def expand(prefix, tree, out):
    """`use` tree `tree` under `prefix` → (path, local name) pairs."""
    tree = tree.strip()
    if tree.startswith("{"):
        depth, cur, parts = 0, "", []
        for ch in tree[1:-1]:
            depth += (ch == "{") - (ch == "}")
            if ch == "," and depth == 0:
                parts.append(cur)
                cur = ""
            else:
                cur += ch
        for part in parts + [cur]:
            if part.strip():
                expand(prefix, part, out)
        return
    m = re.match(r"([\w:]+?)(?:::(\{.*\}))?(?:\s+as\s+(\w+))?$", tree, re.S)
    if not m:
        return
    path, sub, alias = m.groups()
    full = prefix + [x for x in path.split("::") if x]
    if sub:
        expand(full, sub, out)
    elif full:
        if full[-1] == "self":
            full = full[:-1]
        out.append((full, alias or full[-1]))


def rust_uses(groups):
    """(group, NAME) → the crates whose Rust names it."""
    uses = collections.defaultdict(set)
    for path, text in sources(RUST, "rs"):
        in_ids = "/yog-ids/" in path
        who = "yog-ids" if in_ids else owner(path)
        local = {}
        for m in re.finditer(r"\buse\s+([^;]+);", text):
            pairs = []
            expand([], m.group(1), pairs)
            for full, name in pairs:
                if in_ids and full[0] == "crate":
                    full = ["yog_ids"] + full[1:]
                if full[0] in ("yog_ids", "ids"):
                    rest = full[1:]
                    if len(rest) == 1 and rest[0] in groups:
                        local[name] = ("group", rest[0])
                    elif len(rest) == 2 and rest[0] in groups:
                        local[name] = ("name", rest[0], rest[1])
        body = re.sub(r"\buse\s+[^;]+;", "", text)
        for m in re.finditer(r"\b(?:yog_ids|ids|crate)::(\w+)::([A-Z][A-Z0-9_]*)\b", body):
            if m.group(1) in groups:
                uses[(m.group(1), m.group(2))].add(who)
        for name, what in local.items():
            if what[0] == "group":
                for m in re.finditer(r"(?<![\w:])" + name + r"::([A-Z][A-Z0-9_]*)\b", body):
                    uses[(what[1], m.group(1))].add(who)
            elif re.search(r"(?<![\w:])" + name + r"\b", body):
                uses[(what[1], what[2])].add(who)
    return uses


def camel(group, upper):
    words = group.split("_")
    head = words[0].capitalize() if upper else words[0]
    return head + "".join(w.capitalize() for w in words[1:])


def main(argv):
    all_names = names()
    groups = {g for g, _, _ in all_names}
    rust = rust_uses(groups)
    java = "\n".join(t for _, t in sources(JAVA, "java"))
    ts = "\n".join(t for _, t in sources(TS, "ts")) + "\n".join(t for _, t in sources(TS, "tsx"))
    rows = []
    for g, n, v in all_names:
        users = rust.get((g, n), set())
        rows.append({
            "group": g, "name": n, "value": v,
            "crates": sorted(users - {"yog-ids"}),
            "inner": "yog-ids" in users,
            "java": bool(re.search(r"\b" + camel(g, True) + r"\." + n + r"\b", java)),
            "ts": bool(re.search(r"\b" + camel(g, False) + r"\." + n + r"\b", ts)),
        })
    if argv:
        for r in rows:
            if r["group"] in argv:
                print(f"{r['group']}::{r['name']} = {r['value']}  crates={r['crates']} java={r['java']} ts={r['ts']} inner={r['inner']}")
        return 0
    outside = lambda r: r["crates"] or r["java"] or r["ts"]
    print(len(rows), "names")
    print("no use found:", sum(1 for r in rows if not outside(r) and not r["inner"]))
    print("one crate, no Java/TS:", sum(1 for r in rows if len(r["crates"]) == 1 and not r["java"] and not r["ts"]))
    by_group = collections.defaultdict(list)
    for r in rows:
        by_group[r["group"]].append(r)
    for g, rs in sorted(by_group.items()):
        crates = {c for r in rs for c in r["crates"]}
        if len(crates) <= 1 and not any(r["java"] or r["ts"] for r in rs):
            print(f"  group {g} ({len(rs)}): only {sorted(crates)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
