#!/usr/bin/env python3
"""Generate yog.refmap.json from Java mixin source files.

Reads every @Mixin-annotated class under SRCDIR, extracts @Inject / @ModifyArg /
@ModifyVariable / @Redirect annotations, resolves short class names via imports,
and writes an identity refmap (Mojang → Mojang) that ForgeGradle / NeoGradle's
reobfuscateJar can remap to SRG.

Usage:
    python3 gen_refmap.py <src_root> <output_refmap.json>
"""

import json, os, re, sys

# ── regex helpers ────────────────────────────────────────────────────────────

PACKAGE_RE  = re.compile(r'package\s+([\w.]+)\s*;')
IMPORT_RE   = re.compile(r'import\s+(?:static\s+)?([\w.*]+)\s*;')
CLASS_RE    = re.compile(r'(?:public\s+)?(?:abstract\s+)?class\s+(\w+)')

# @Mixin(Target.class)  or  @Mixin(value = Target.class)  or  @Mixin({A.class, B.class})
MIXIN_RE = re.compile(
    r'@Mixin\s*\(\s*(?:value\s*=\s*)?'
    r'\{?([^}]*)\}?\s*\)',
    re.DOTALL,
)

# @Inject(method = "name", ...)  — multi-line aware
INJECT_RE = re.compile(
    r'@(?:Inject|ModifyArg|ModifyVariable|Redirect)\s*\([^@]*?'
    r'method\s*=\s*"([^"]*)"',
    re.DOTALL,
)

# ── helpers ──────────────────────────────────────────────────────────────────

def class_to_internal(fqcn: str) -> str:
    return "L" + fqcn.replace(".", "/") + ";"

def resolve_class(short: str, imports: list[str], package: str) -> str:
    """Resolve a short class name (or FQCN) to a fully-qualified name."""
    # Already a FQCN?
    if "." in short:
        return short
    # java.lang is auto-imported
    for lang_cls in ("String", "Object", "Integer", "Long", "Float", "Double",
                     "Boolean", "Byte", "Short", "Character", "Void"):
        if short == lang_cls:
            return "java.lang." + short
    # Check imports
    for imp in imports:
        if imp.endswith("." + short):
            return imp
        if imp.endswith(".*"):
            # We can't resolve wildcard imports precisely; assume same package
            pass
    # Same-package fallback
    return package + "." + short if package else short

def extract_class_names(text: str) -> list[str]:
    """Extract comma-separated class names from @Mixin arguments like
    'Target.class' or 'value = {Target1.class, Target2.class}'."""
    # Remove .class and strip whitespace
    parts = re.findall(r'([\w.]+)\.class', text)
    return parts

def parse_source(path: str) -> dict | None:
    """Parse a Java source file, return {fqcn, target_fqcn, methods} or None."""
    with open(path, encoding="utf-8") as f:
        text = f.read()

    pkg_m  = PACKAGE_RE.search(text)
    cls_m  = CLASS_RE.search(text)
    if not pkg_m or not cls_m:
        return None

    package = pkg_m.group(1)
    fqcn    = package + "." + cls_m.group(1)

    # Collect imports
    imports = [m.group(1) for m in IMPORT_RE.finditer(text)]

    # Find @Mixin
    mixin_m = MIXIN_RE.search(text)
    if not mixin_m:
        return None

    targets = extract_class_names(mixin_m.group(0))
    if not targets:
        return None
    target_fqcn = resolve_class(targets[0], imports, package)

    # Find @Inject etc.
    entries = {}
    for m in INJECT_RE.finditer(text):
        method_sig = m.group(1).strip()
        internal_target = class_to_internal(target_fqcn)
        entries[method_sig] = internal_target + method_sig

    if entries:
        return {"fqcn": fqcn, "methods": entries}
    return None

def scan_sources(src_root: str) -> dict:
    """Walk src_root, parse mixin classes, return refmap dict."""
    mappings = {}
    for dirpath, _, filenames in os.walk(src_root):
        for fn in filenames:
            if not fn.endswith(".java"):
                continue
            path = os.path.join(dirpath, fn)
            result = parse_source(path)
            if result:
                mappings[result["fqcn"]] = result["methods"]
    return {"mappings": mappings}

def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <src_root> <output.json>", file=sys.stderr)
        sys.exit(1)
    src_root = sys.argv[1]
    out_path = sys.argv[2]
    refmap = scan_sources(src_root)
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(refmap, f, indent=2)
    n = len(refmap.get("mappings", {}))
    print(f"  gen_refmap: wrote {out_path} ({n} mixins)")

if __name__ == "__main__":
    main()
