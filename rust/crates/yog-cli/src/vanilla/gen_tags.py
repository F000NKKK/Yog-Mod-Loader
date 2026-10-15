"""The tags every Minecraft version and loader Yog supports has, and Yog's
common tags (`yog:`) over them — for yog-cli's checks and the hosts
(java-common's yog/tags.txt is a copy). Run with the client and loader jars
in Gradle's caches (build every host once):

    python3 gen_tags.py tags.txt

Add a version or loader to HOSTS when Yog starts supporting it."""
import collections, glob, os, re, sys, zipfile

CACHE = os.path.expanduser("~/.gradle/caches")
VERSIONS = ["1.20.1", "1.21.1"]
# loader/version → the jars whose data/ has its tags (beside the game's).
HOSTS = {
    "fabric/1.20.1": ["modules-2/files-2.1/net.fabricmc.fabric-api/fabric-convention-tags-v1/1.5.5+*/*/fabric-convention-tags-v1-1.5.5+*[0-9a-f].jar"],
    "fabric/1.21.1": ["modules-2/files-2.1/net.fabricmc.fabric-api/fabric-convention-tags-v2/2.11.0+*/*/fabric-convention-tags-v2-2.11.0+*[0-9a-f].jar"],
    "forge/1.20.1": ["forge_gradle/maven_downloader/net/minecraftforge/forge/1.20.1-47.3.0/forge-1.20.1-47.3.0-universal.jar"],
    "forge/1.21.1": ["forge_gradle/maven_downloader/net/minecraftforge/forge/1.21.1-52.0.40/forge-1.21.1-52.0.40-universal-srg.jar"],
    "neoforge/1.20.1": ["modules-2/files-2.1/net.neoforged/forge/1.20.1-47.1.106/*/forge-1.20.1-47.1.106-universal.jar"],
    "neoforge/1.21.1": ["modules-2/files-2.1/net.neoforged/neoforge/21.1.77/*/neoforge-21.1.77-universal.jar"],
}
LOADERS = sorted({h.split("/")[0] for h in HOSTS})
# 1.20's plural tag folders.
SINGULAR = {"items": "item", "blocks": "block", "entity_types": "entity_type", "fluids": "fluid", "game_events": "game_event"}


def tags_in(path):
    """(registry, id) of every tag file in a jar."""
    out = set()
    for n in zipfile.ZipFile(path).namelist():
        m = re.fullmatch(r"data/([a-z0-9_.-]+)/tags/([a-z0-9_/]+)\.json", n)
        if not m:
            continue
        parts = m[2].split("/")
        k = 2 if parts[0] == "worldgen" else 1
        if len(parts) <= k:
            continue
        reg = SINGULAR.get(parts[0], "/".join(parts[:k]))
        out.add((reg, f"{m[1]}:{'/'.join(parts[k:])}"))
    return out


where = collections.defaultdict(set)  # (registry, id) → hosts
for host, jars in HOSTS.items():
    version = host.split("/")[1]
    found = tags_in(os.path.join(CACHE, f"fabric-loom/{version}/minecraft-client.jar"))
    for pattern in jars:
        paths = glob.glob(os.path.join(CACHE, pattern))
        if not paths:
            sys.exit(f"no {pattern} — build the {host} host once")
        found |= tags_in(paths[0])
    for t in found:
        where[t].add(host)


def place(hosts):
    """`*`, the versions every loader has it on, and single hosts."""
    if len(hosts) == len(HOSTS):
        return "*"
    out = []
    for v in VERSIONS:
        on = {h for h in hosts if h.endswith("/" + v)}
        if len(on) == len(LOADERS):
            out.append(v)
        else:
            out.extend(sorted(on))
    return ",".join(out)


# Yog's names are the 1.21 common (`c:`) tags' and the game's; the other
# conventions name some of them otherwise.
FORGE_1_20 = {  # forge:<old> on 1.20 → the 1.21 name
    "cobblestone": "cobblestones", "sand": "sands", "gravel": "gravels", "glass": "glass_blocks",
    "stone": "stones", "netherrack": "netherracks", "obsidian": "obsidians", "end_stones": "end_stones",
    "sandstone": "sandstone/blocks", "chests/wooden": "chests/wooden", "glass/colorless": "glass_blocks/colorless",
    "glass/tinted": "glass_blocks/tinted", "glass_panes/colorless": "glass_panes/colorless",
    "tools/bows": "tools/bow", "tools/crossbows": "tools/crossbow", "tools/fishing_rods": "tools/fishing_rod",
    "tools/shields": "tools/shield", "tools/tridents": "tools/spear", "armors": "armors",
    "eggs": "eggs", "feathers": "feathers", "gunpowder": "gunpowders", "leather": "leathers",
    "string": "strings", "slimeballs": "slime_balls", "bones": "bones", "heads": "heads",
    "mushrooms": "mushrooms", "nether_stars": "nether_stars", "ender_pearls": "ender_pearls",
    "enchanting_fuels": "enchanting_fuels", "shears": "tools/shear", "crops/nether_wart": "crops/nether_wart",
}
FABRIC_1_20 = {  # c:<old> on fabric 1.20 → the 1.21 name
    "diamonds": "gems/diamond", "emeralds": "gems/emerald", "lapis": "gems/lapis", "quartz": "gems/quartz",
    "coal": "coal", "glass_blocks": "glass_blocks", "glass_panes": "glass_panes",
    "raw_copper_blocks": "storage_blocks/raw_copper", "raw_gold_blocks": "storage_blocks/raw_gold",
    "raw_iron_blocks": "storage_blocks/raw_iron", "raw_copper_ores": "raw_materials/copper",
    "raw_gold_ores": "raw_materials/gold", "raw_iron_ores": "raw_materials/iron", "raw_ores": "raw_materials",
    "quartz_ores": "ores/quartz", "sandstone_blocks": "sandstone/blocks", "sandstone_slabs": "sandstone/slabs",
    "sandstone_stairs": "sandstone/stairs", "red_sandstone_blocks": "sandstone/red_blocks",
    "red_sandstone_slabs": "sandstone/red_slabs", "red_sandstone_stairs": "sandstone/red_stairs",
    "uncolored_sandstone_blocks": "sandstone/uncolored_blocks", "uncolored_sandstone_slabs": "sandstone/uncolored_slabs",
    "uncolored_sandstone_stairs": "sandstone/uncolored_stairs", "wooden_barrels": "barrels/wooden",
    "axes": "tools/axe", "hoes": "tools/hoe", "pickaxes": "tools/pickaxe", "shovels": "tools/shovel",
    "swords": "tools/sword", "bows": "tools/bow", "shields": "tools/shield", "spears": "tools/spear",
    "shears": "tools/shear", "empty_buckets": "buckets/empty", "water_buckets": "buckets/water",
    "lava_buckets": "buckets/lava", "milk_buckets": "buckets/milk", "entity_water_buckets": "buckets/entity_water",
    "budding_blocks": "budding_blocks", "buds": "buds", "clusters": "clusters", "wood_sticks": "rods/wooden",
}


def modern(ns, path, hosts):
    """The 1.21 name of a convention tag `ns:path` of `hosts`."""
    if ns == "forge":
        return FORGE_1_20.get(path, path)
    if ns == "c" and not any(h.endswith("/1.21.1") for h in hosts):
        if True:
            if path in FABRIC_1_20:
                return FABRIC_1_20[path]
            # `iron_ingots` → `ingots/iron`, `brown_dyes` → `dyes/brown`.
            m = re.fullmatch(r"([a-z0-9_]+?)_([a-z]+s)", path)
            if m:
                return f"{m[2]}/{m[1]}"
    return path


yog = collections.defaultdict(list)  # (registry, yog path) → its tags, the game's first
for (reg, tid), hosts in sorted(where.items()):
    ns, path = tid.split(":", 1)
    if ns == "minecraft":
        name = path
    elif ns in ("c", "forge"):
        name = modern(ns, path, hosts)
    else:
        continue
    yog[(reg, name)].append(tid)
for k in yog:
    yog[k].sort(key=lambda t: (not t.startswith("minecraft:"), not t.startswith("c:"), t))

out = [
    "# Tags of every Minecraft version and loader Yog supports, and Yog's common tags.",
    "# Generated by gen_tags.py from the game's and the loaders' jars; java-common's yog/tags.txt is a copy.",
    "# versions: " + " ".join(VERSIONS),
    "# loaders: " + " ".join(LOADERS),
    "# tag <registry> <id> <where>: * — everywhere; a version — every loader of it; <loader>/<version>.",
    "# yog <registry> <path> <tag>...: yog:<path> is these tags — those a host has; a mod's entries go to the first.",
]
for (reg, tid), hosts in sorted(where.items()):
    out.append(f"tag {reg} {tid} {place(hosts)}")
for (reg, name), tags in sorted(yog.items()):
    out.append(f"yog {reg} {name} {' '.join(tags)}")
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print(len(where), "tags,", len(yog), "yog tags")
