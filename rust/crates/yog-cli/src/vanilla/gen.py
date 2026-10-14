"""Vanilla content ids per Minecraft version, for yog-cli's reference checks.
Run with the client jars of every supported version (Loom's cache):

    python3 gen.py ids.txt

Add a version to VERSIONS when Yog starts supporting it."""
import zipfile, json, re, os, sys, collections
VERSIONS = ["1.20.1", "1.21.1"]
ids = collections.defaultdict(set)   # (kind, id) -> versions
for v in VERSIONS:
    z = zipfile.ZipFile(os.path.expanduser(f"~/.gradle/caches/fabric-loom/{v}/minecraft-client.jar"))
    names = z.namelist()
    lang = json.loads(z.read("assets/minecraft/lang/en_us.json"))
    named_items = {k.split(".")[2] for k in lang if re.fullmatch(r"item\.minecraft\.[a-z0-9_]+", k)}
    named_blocks = {k.split(".")[2] for k in lang if re.fullmatch(r"block\.minecraft\.[a-z0-9_]+", k)}
    # An item model is an item's when the item has a name or its own texture
    # (1.20.1's smithing templates share one name, `item.minecraft.smithing_template`);
    # template models (`generated`, `template_*`) have neither.
    # Models an item switches to (`bow_pulling_0`, `clock_01`: its `overrides`) aren't items.
    textured = {m[1] for n in names if (m := re.fullmatch(r"assets/minecraft/textures/item/([a-z0-9_]+)\.png", n))}
    for n in names:
        if re.fullmatch(r"assets/minecraft/models/item/[a-z0-9_]+\.json", n):
            for o in json.loads(z.read(n)).get("overrides", []):
                textured.discard(o.get("model", "").split("/")[-1])
    for n in names:
        m = re.fullmatch(r"assets/minecraft/models/item/([a-z0-9_]+)\.json", n)
        if m and (m[1] in named_items or m[1] in named_blocks or m[1] in textured):
            ids[("item", m[1])].add(v)
        m = re.fullmatch(r"assets/minecraft/blockstates/([a-z0-9_]+)\.json", n)
        if m:
            ids[("block", m[1])].add(v)
        m = re.fullmatch(r"data/minecraft/recipes?/([a-z0-9_/]+)\.json", n)
        if m:
            ids[("recipe", m[1])].add(v)
    for k in lang:
        m = re.fullmatch(r"entity\.minecraft\.([a-z0-9_]+)", k)
        if m:
            ids[("entity", m[1])].add(v)
out = ["# Vanilla ids per Minecraft version: <kind> <path> <versions>; generated from the client jars.",
       "# versions: " + " ".join(VERSIONS)]
for (kind, path), vs in sorted(ids.items()):
    tag = "*" if len(vs) == len(VERSIONS) else ",".join(sorted(vs))
    out.append(f"{kind} {path} {tag}")
open(sys.argv[1], "w").write("\n".join(out) + "\n")
c = collections.Counter(k for k, _ in ids)
print(c, "partial:", sum(1 for vs in ids.values() if len(vs) < len(VERSIONS)))
