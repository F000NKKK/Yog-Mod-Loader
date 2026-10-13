//! `yog import` — turns models made elsewhere into `content/` files:
//!
//! - `yog import resourcepack <zip|dir> [--model <id>] [--all]` — a
//!   resource pack's (or a mod's `assets/`) block and item models: parents
//!   resolved (a vanilla template stays a template, other parents are
//!   flattened from the pack or the Minecraft client jar), textures copied
//!   into `resources/`, the blockstate's rotations kept;
//! - `yog import bbmodel <file.bbmodel> [--id <name>] [--item]` — a
//!   Blockbench project in the Java block/item format, textures included.

use super::json::{to_pretty, J, V};
use super::{template, valid_namespace, valid_path, TEMPLATES};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage:\n  yog import resourcepack <pack.zip|dir> [--model <ns:block/name|ns:item/name>] [--all] [--force]\n  yog import bbmodel <file.bbmodel> [--id <name>] [--item] [--force]";

/// `yog import ...` (args after `import`).
pub fn import_command(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let positional: Vec<&str> = {
        let mut out = Vec::new();
        let mut skip = false;
        for a in args.iter().skip(1) {
            if skip {
                skip = false;
            } else if a == "--model" || a == "--id" {
                skip = true;
            } else if !a.starts_with("--") {
                out.push(a.as_str());
            }
        }
        out
    };
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    let mod_id = read_mod_id(&root)?;
    let mut target = Target {
        root: root.clone(),
        ns: valid_namespace(&mod_id).then_some(mod_id),
        force: flag("--force"),
        written: Vec::new(),
        warnings: Vec::new(),
    };
    match (args.first().map(String::as_str), positional.first()) {
        (Some("resourcepack"), Some(src)) => {
            let pack = Pack::open(&root.join(src))?;
            import_pack(&mut target, &pack, value("--model"), flag("--all"))?;
        }
        (Some("bbmodel"), Some(file)) => {
            import_bbmodel(&mut target, &root.join(file), value("--id"), flag("--item"))?;
        }
        _ => return Err(USAGE.into()),
    }
    for w in &target.warnings {
        eprintln!("warning: {w}");
    }
    for f in &target.written {
        eprintln!("==> wrote {f}");
    }
    if target.written.is_empty() {
        return Err("nothing imported".into());
    }
    eprintln!("    `yog content check` validates them; register with `yog_api::content!();` in src/lib.rs");
    Ok(())
}

fn read_mod_id(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join("yog.toml")).map_err(|_| {
        "no yog.toml in the current directory — run `yog import` in a mod".to_string()
    })?;
    let mut in_mod = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_mod = t == "[mod]";
        } else if in_mod {
            if let Some(v) = t
                .strip_prefix("id")
                .map(str::trim_start)
                .and_then(|r| r.strip_prefix('='))
            {
                return Ok(v.trim().trim_matches('"').to_string());
            }
        }
    }
    Err("yog.toml has no [mod] id".into())
}

/// Where imports go.
struct Target {
    root: PathBuf,
    /// The mod's namespace, when its id is one.
    ns: Option<String>,
    force: bool,
    written: Vec<String>,
    warnings: Vec<String>,
}

impl Target {
    /// The `"id"` to write for `name` from namespace `from_ns`: none when
    /// the mod's namespace applies.
    fn id_for(&self, from_ns: &str, name: &str) -> Option<String> {
        self.ns.is_none().then(|| format!("{from_ns}:{name}"))
    }

    /// The namespace textures bundled for an entry of `from_ns` land in.
    fn texture_ns(&self, from_ns: &str) -> String {
        self.ns.clone().unwrap_or_else(|| from_ns.to_string())
    }

    fn write_file(&mut self, rel: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.root.join(rel);
        if path.exists() && !self.force {
            if std::fs::read(&path).ok().as_deref() == Some(bytes) {
                return Ok(());
            }
            return Err(format!("{rel} already exists (--force overwrites)"));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{rel}: {e}"))?;
        self.written.push(rel.to_string());
        Ok(())
    }

    /// Copies a texture into `resources/<path>.png` → its value in a
    /// content file (`path.png`).
    fn put_texture(
        &mut self,
        path: &str,
        png: &[u8],
        mcmeta: Option<&[u8]>,
    ) -> Result<String, String> {
        let rel = format!("resources/{path}.png");
        let existing = self.root.join(&rel);
        if !(existing.is_file() && std::fs::read(&existing).ok().as_deref() == Some(png)) {
            let force = self.force;
            // A texture already there and different: keep it, warn.
            if existing.is_file() && !force {
                self.warnings.push(format!(
                    "{rel} exists and differs — kept it (--force overwrites)"
                ));
            } else {
                self.write_file(&rel, png)?;
            }
        }
        if let Some(meta) = mcmeta {
            let rel = format!("resources/{path}.png.mcmeta");
            if !self.root.join(&rel).is_file() || self.force {
                self.write_file(&rel, meta)?;
            }
        }
        Ok(format!("{path}.png"))
    }
}

// ── Resource packs ───────────────────────────────────────────────────────────

/// A zip or a directory with `assets/`.
enum Pack {
    Dir(PathBuf),
    Zip(BTreeMap<String, Vec<u8>>),
}

impl Pack {
    fn open(path: &Path) -> Result<Pack, String> {
        if path.is_dir() {
            if !path.join("assets").is_dir() {
                return Err(format!("{} has no assets/ folder", path.display()));
            }
            return Ok(Pack::Dir(path.to_path_buf()));
        }
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut files = BTreeMap::new();
        // Packs sometimes wrap everything in one folder.
        let mut prefix: Option<String> = None;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().map_err(|e| e.to_string())?.replace('\\', "/");
            if prefix.is_none() {
                if let Some(at) = name.find("assets/") {
                    prefix = Some(name[..at].to_string());
                }
            }
            if !name.ends_with(".json") && !name.ends_with(".png") && !name.ends_with(".mcmeta") {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            files.insert(name, bytes);
        }
        let prefix = prefix.unwrap_or_default();
        Ok(Pack::Zip(
            files
                .into_iter()
                .filter_map(|(k, v)| k.strip_prefix(&prefix).map(|k| (k.to_string(), v)))
                .collect(),
        ))
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        match self {
            Pack::Dir(d) => std::fs::read(d.join(rel)).ok(),
            Pack::Zip(files) => files.get(rel).cloned(),
        }
    }

    /// Paths under `assets/` matching `assets/<ns>/<dir>/<name>.json` → (ns, name).
    fn list(&self, dir: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut add = |rel: &str| {
            let rest = rel.strip_prefix("assets/")?;
            let (ns, rest) = rest.split_once('/')?;
            let name = rest
                .strip_prefix(dir)?
                .strip_prefix('/')?
                .strip_suffix(".json")?;
            out.push((ns.to_string(), name.to_string()));
            Some(())
        };
        match self {
            Pack::Dir(d) => {
                let mut stack = vec![d.join("assets")];
                while let Some(p) = stack.pop() {
                    for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
                        let path = e.path();
                        if path.is_dir() {
                            stack.push(path);
                        } else if let Ok(rel) = path.strip_prefix(d) {
                            add(&rel.to_string_lossy().replace('\\', "/"));
                        }
                    }
                }
            }
            Pack::Zip(files) => {
                for k in files.keys() {
                    add(k);
                }
            }
        }
        out.sort();
        out
    }
}

/// The newest Minecraft client jar on this machine (vanilla models for
/// flattening parents).
fn client_jar(root: &Path) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("YOG_MC_CLIENT_JAR") {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let mut dirs: Vec<(PathBuf, &str)> = Vec::new();
    if let Some(h) = &home {
        let caches = h.join(".gradle/caches");
        dirs.push((caches.join("fabric-loom"), "minecraft-client.jar"));
        dirs.push((
            caches.join("forge_gradle/minecraft_repo/versions"),
            "client.jar",
        ));
        dirs.push((caches.join("minecraft/versions"), "client.jar"));
        dirs.push((h.join(".minecraft/versions"), ""));
    }
    for loader in ["neoforge", "fabric", "forge"] {
        dirs.push((
            root.join("../Yog-Mod-Loader")
                .join(loader)
                .join(".gradle/caches/minecraft/versions"),
            "client.jar",
        ));
    }
    let key = |v: &str| -> Vec<u32> {
        v.split(['.', '-'])
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    for (dir, file) in dirs {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let v = e.file_name().to_string_lossy().into_owned();
            if !v.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            let jar = if file.is_empty() {
                e.path().join(format!("{v}.jar"))
            } else {
                e.path().join(file)
            };
            if jar.is_file() && best.as_ref().is_none_or(|(k, _)| key(&v) > *k) {
                best = Some((key(&v), jar));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Reads models: the pack first, then the client jar.
struct Models<'a> {
    pack: &'a Pack,
    jar: Option<zip::ZipArchive<std::fs::File>>,
    jar_tried: bool,
    root: PathBuf,
}

impl Models<'_> {
    fn model(&mut self, id: &str) -> Option<(Map<String, Value>, bool)> {
        let (ns, path) = split(id);
        let rel = format!("assets/{ns}/models/{path}.json");
        if let Some(b) = self.pack.read(&rel) {
            return serde_json::from_slice(&b).ok().map(|m| (m, true));
        }
        if !self.jar_tried {
            self.jar_tried = true;
            self.jar = client_jar(&self.root)
                .and_then(|p| std::fs::File::open(p).ok())
                .and_then(|f| zip::ZipArchive::new(f).ok());
        }
        let jar = self.jar.as_mut()?;
        let mut entry = jar.by_name(&rel).ok()?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;
        serde_json::from_slice(&bytes).ok().map(|m| (m, false))
    }
}

/// `ns:path` / `path` (minecraft).
fn split(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

fn full_id(id: &str) -> String {
    let (ns, p) = split(id);
    format!("{ns}:{p}")
}

/// The template a vanilla model id is.
fn template_of(id: &str) -> Option<&'static super::Template> {
    let id = full_id(id);
    TEMPLATES.iter().find(|t| t.parent == id)
}

/// A model of the pack → a content model object (and `render_type`).
fn convert_model(
    t: &mut Target,
    models: &mut Models,
    pack: &Pack,
    id: &str,
    item: bool,
) -> Result<(J, Option<String>), String> {
    let mut textures: Vec<(String, String)> = Vec::new();
    let mut elements: Option<Vec<Value>> = None;
    let mut display: Map<String, Value> = Map::new();
    let mut ao: Option<bool> = None;
    let mut gui_light: Option<String> = None;
    let mut render_type: Option<String> = None;
    let mut parent: Option<String> = None;
    let mut current = full_id(id);
    let mut from_ns = split(&current).0.to_string();
    for depth in 0.. {
        if depth > 16 {
            return Err(format!("{id}: parent chain too long"));
        }
        let Some((m, _)) = models.model(&current) else {
            if depth == 0 {
                return Err(format!("{id}: no such model in the pack"));
            }
            t.warnings.push(format!(
                "{id}: parent {current} not found (no Minecraft client jar?) — kept as the parent"
            ));
            parent = Some(current.clone());
            break;
        };
        if depth == 0 {
            from_ns = split(&current).0.to_string();
        }
        if let Some(Value::Object(tx)) = m.get("textures") {
            for (k, v) in tx {
                if !textures.iter().any(|(s, _)| s == k) {
                    if let Some(s) = v.as_str() {
                        textures.push((k.clone(), s.to_string()));
                    }
                }
            }
        }
        if elements.is_none() {
            if let Some(Value::Array(e)) = m.get("elements") {
                elements = Some(e.clone());
            }
        }
        if let Some(Value::Object(d)) = m.get("display") {
            for (k, v) in d {
                display.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        if ao.is_none() {
            ao = m.get("ambientocclusion").and_then(Value::as_bool);
        }
        if gui_light.is_none() {
            gui_light = m.get("gui_light").and_then(Value::as_str).map(String::from);
        }
        if render_type.is_none() {
            render_type = m
                .get("render_type")
                .and_then(Value::as_str)
                .map(|s| s.trim_start_matches("minecraft:").to_string());
        }
        let Some(p) = m.get("parent").and_then(Value::as_str) else {
            break;
        };
        if p.starts_with("builtin/") || p.starts_with("minecraft:builtin/") {
            if p.ends_with("generated") {
                parent = Some("generated".into());
                break;
            }
            return Err(format!("{id}: built-in model {p} can't be imported"));
        }
        if elements.is_none() {
            if let Some(tpl) = template_of(p) {
                if !tpl.sprite || item {
                    parent = Some(tpl.name.to_string());
                    break;
                }
            }
        }
        current = full_id(p);
    }
    // A sprite parent reached through the jar: `item/generated` itself.
    if elements.is_none() && parent.is_none() {
        parent = Some(if item {
            "generated".into()
        } else {
            "cube_all".into()
        });
    }

    let mut out = J::obj::<&str>([]);
    if let Some(p) = &parent {
        out.set("parent", J::string(p.clone()));
    }
    // Textures that are references to other slots stay; ids get copied
    // when the pack has them.
    let mut tex = J::obj::<&str>([]);
    let tex_ns = t.texture_ns(&from_ns);
    for (slot, v) in &textures {
        let value = if v.starts_with('#') {
            v.clone()
        } else {
            let (ns, path) = split(v);
            let rel = format!("assets/{ns}/textures/{path}.png");
            match pack.read(&rel) {
                Some(png) if valid_path(path) => {
                    let meta = pack.read(&format!("{rel}.mcmeta"));
                    if ns != tex_ns {
                        t.warnings.push(format!(
                            "{id}: texture {ns}:{path} is bundled as {tex_ns}:{path}"
                        ));
                    }
                    t.put_texture(path, &png, meta.as_deref())?
                }
                _ => full_id(v),
            }
        };
        tex.set(slot, J::string(value));
    }
    // Templates already define `particle` — drop a duplicate of a slot.
    if let Some(tpl) = parent.as_deref().and_then(template) {
        if tpl.has_particle {
            if let V::Obj(m) = &mut tex.v {
                m.retain(|(k, _)| k != "particle");
            }
        }
    }
    if !tex.members().is_empty() {
        out.set("textures", tex);
    }
    if let Some(els) = &elements {
        let converted: Vec<J> = els
            .iter()
            .enumerate()
            .map(|(i, e)| element_from_mc(t, id, i, e))
            .collect();
        out.set("elements", J::of(V::Arr(converted)));
    }
    if !display.is_empty() {
        out.set("display", J::from_value(&Value::Object(display)));
    }
    if let Some(ao) = ao {
        out.set("ambient_occlusion", J::of(V::Bool(ao)));
    }
    if let Some(g) = gui_light {
        out.set("gui_light", J::string(g));
    }
    Ok((out, render_type))
}

/// The nearest allowed element angle (-45..45 by 22.5).
fn snap_angle(t: &mut Target, what: &str, a: f64) -> f64 {
    let snapped = ((a / 22.5).round() * 22.5).clamp(-45.0, 45.0);
    if (snapped - a).abs() > 1e-6 {
        t.warnings.push(format!(
            "{what}: rotation {a}° → {snapped}° (Minecraft allows -45..45 by 22.5)"
        ));
    }
    snapped
}

fn clamp_coord(t: &mut Target, what: &str, v: f64) -> f64 {
    let c = v.clamp(-16.0, 32.0);
    if c != v {
        t.warnings
            .push(format!("{what}: {v} is outside -16..32, clamped"));
    }
    (c * 10000.0).round() / 10000.0
}

/// A Minecraft model element → a content element (same shape, `tint`).
fn element_from_mc(t: &mut Target, id: &str, i: usize, e: &Value) -> J {
    let what = format!("{id} element {i}");
    let coords = |t: &mut Target, v: Option<&Value>| -> Vec<f64> {
        let a: Vec<f64> = v
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        a.into_iter().map(|n| clamp_coord(t, &what, n)).collect()
    };
    let mut out = J::obj::<&str>([]);
    if let Some(n) = e.get("name").and_then(Value::as_str) {
        out.set("name", J::string(n));
    }
    out.set("from", J::nums(&coords(t, e.get("from"))));
    out.set("to", J::nums(&coords(t, e.get("to"))));
    if let Some(r) = e.get("rotation") {
        let angle = r.get("angle").and_then(Value::as_f64).unwrap_or(0.0);
        let angle = snap_angle(t, &what, angle);
        if angle != 0.0 {
            let mut rot = J::obj([(
                "axis",
                J::string(r.get("axis").and_then(Value::as_str).unwrap_or("y")),
            )]);
            rot.set("angle", J::number(angle));
            rot.set("origin", J::nums(&coords(t, r.get("origin"))));
            if r.get("rescale").and_then(Value::as_bool) == Some(true) {
                rot.set("rescale", J::of(V::Bool(true)));
            }
            out.set("rotation", rot);
        }
    }
    if e.get("shade").and_then(Value::as_bool) == Some(false) {
        out.set("shade", J::of(V::Bool(false)));
    }
    let mut faces = J::obj::<&str>([]);
    if let Some(Value::Object(f)) = e.get("faces") {
        for dir in super::DIRECTIONS {
            let Some(face) = f.get(dir) else { continue };
            let mut fo = J::obj([(
                "texture",
                J::string(
                    face.get("texture")
                        .and_then(Value::as_str)
                        .unwrap_or("#missing"),
                ),
            )]);
            if let Some(uv) = face.get("uv").and_then(Value::as_array) {
                let uv: Vec<f64> = uv
                    .iter()
                    .filter_map(Value::as_f64)
                    .map(|n| (n.clamp(0.0, 16.0) * 10000.0).round() / 10000.0)
                    .collect();
                fo.set("uv", J::nums(&uv));
            }
            if let Some(r) = face
                .get("rotation")
                .and_then(Value::as_f64)
                .filter(|r| *r != 0.0)
            {
                fo.set("rotation", J::number(r));
            }
            if let Some(c) = face.get("cullface").and_then(Value::as_str) {
                fo.set(
                    "cullface",
                    J::string(if c == "bottom" { "down" } else { c }),
                );
            }
            if let Some(n) = face.get("tintindex").and_then(Value::as_f64) {
                fo.set("tint", J::number(n));
            }
            faces.set(dir, fo);
        }
    }
    out.set("faces", faces);
    out
}

/// The blockstate's rotations (`"variants": {"": [...]}`) — or none.
fn state_rotations(t: &mut Target, id: &str, bs: &Value) -> (Option<String>, Option<J>) {
    let one = |v: &Value| -> (Option<String>, J) {
        let mut r = J::obj::<&str>([]);
        for k in ["x", "y"] {
            if let Some(n) = v.get(k).and_then(Value::as_f64).filter(|n| *n != 0.0) {
                r.set(k, J::number(n));
            }
        }
        if let Some(w) = v.get("weight").and_then(Value::as_f64) {
            r.set("weight", J::number(w));
        }
        if v.get("uvlock").and_then(Value::as_bool) == Some(true) {
            r.set("uvlock", J::of(V::Bool(true)));
        }
        (v.get("model").and_then(Value::as_str).map(String::from), r)
    };
    let variant = if let Some(Value::Object(vs)) = bs.get("variants") {
        if vs.len() > 1 {
            t.warnings.push(format!(
                "{id}: the blockstate has {} variants (block state properties) — only the first is imported; content files don't do properties yet",
                vs.len()
            ));
        }
        vs.values().next().cloned()
    } else if let Some(Value::Array(parts)) = bs.get("multipart") {
        t.warnings.push(format!(
            "{id}: multipart blockstate — only the first part is imported"
        ));
        parts.first().and_then(|p| p.get("apply")).cloned()
    } else {
        None
    };
    match variant {
        Some(Value::Array(list)) => {
            let mut model = None;
            let mut rots = Vec::new();
            for v in &list {
                let (m, r) = one(v);
                model = model.or(m);
                rots.push(r);
            }
            (model, Some(J::of(V::Arr(rots))))
        }
        Some(v) => {
            let (m, r) = one(&v);
            let rots = (!r.members().is_empty()).then(|| J::of(V::Arr(vec![r])));
            (m, rots)
        }
        None => (None, None),
    }
}

/// The pack's English name for a key, if it has a plain `en_us.json`.
fn lang_name(pack: &Pack, ns: &str, key: &str) -> Option<String> {
    let bytes = pack.read(&format!("assets/{ns}/lang/en_us.json"))?;
    let m: Map<String, Value> = serde_json::from_slice(&bytes).ok()?;
    m.get(key).and_then(Value::as_str).map(String::from)
}

fn header(t: &Target, ns: &str, name: &str, pack: &Pack, kind: &str) -> J {
    let mut j = J::obj::<&str>([]);
    if let Some(id) = t.id_for(ns, name) {
        j.set("id", J::string(id));
    }
    if let Some(n) = lang_name(pack, ns, &format!("{kind}.{ns}.{}", name.replace('/', "."))) {
        j.set("name", J::string(n));
    }
    j
}

fn import_block(
    t: &mut Target,
    models: &mut Models,
    pack: &Pack,
    ns: &str,
    name: &str,
) -> Result<(), String> {
    let bs: Option<Value> = pack
        .read(&format!("assets/{ns}/blockstates/{name}.json"))
        .and_then(|b| serde_json::from_slice(&b).ok());
    let (model_id, rotations) = match &bs {
        Some(bs) => state_rotations(t, &format!("{ns}:{name}"), bs),
        None => (None, None),
    };
    let model_id = model_id.unwrap_or_else(|| format!("{ns}:block/{name}"));
    let (model, render) = convert_model(t, models, pack, &model_id, false)?;
    let mut j = header(t, ns, name, pack, "block");
    if let Some(r) = render.filter(|r| super::RENDER_LAYERS.contains(&r.as_str()) && r != "solid") {
        j.set("block", J::obj([("render_layer", J::string(r))]));
    }
    j.set("model", model);
    // Its own item model, unless it's just the block's.
    if let Some((im, _)) = models.model(&format!("{ns}:item/{name}")) {
        let plain = im.len() == 1
            && im.get("parent").and_then(Value::as_str).map(full_id) == Some(full_id(&model_id));
        if !plain {
            let (item_model, _) =
                convert_model(t, models, pack, &format!("{ns}:item/{name}"), true)?;
            j.set("item_model", item_model);
        }
    }
    if let Some(r) = rotations {
        j.set("states", J::obj([("rotations", r)]));
    }
    let leaf = name.rsplit('/').next().unwrap_or(name);
    t.write_file(
        &format!("content/blocks/{leaf}.yblock"),
        to_pretty(&j).as_bytes(),
    )
}

fn import_item(
    t: &mut Target,
    models: &mut Models,
    pack: &Pack,
    ns: &str,
    name: &str,
) -> Result<(), String> {
    let (model, _) = convert_model(t, models, pack, &format!("{ns}:item/{name}"), true)?;
    let mut j = header(t, ns, name, pack, "item");
    j.set("model", model);
    let leaf = name.rsplit('/').next().unwrap_or(name);
    t.write_file(
        &format!("content/items/{leaf}.yitem"),
        to_pretty(&j).as_bytes(),
    )
}

fn import_pack(t: &mut Target, pack: &Pack, model: Option<&str>, all: bool) -> Result<(), String> {
    let mut models = Models {
        pack,
        jar: None,
        jar_tried: false,
        root: t.root.clone(),
    };
    let blockstates = pack.list("blockstates");
    let block_models = pack.list("models/block");
    let item_models = pack.list("models/item");
    if let Some(m) = model {
        let (ns, path) = split(m);
        let ns = if m.contains(':') {
            ns.to_string()
        } else {
            t.ns.clone().unwrap_or_else(|| "minecraft".into())
        };
        let path = if m.contains(':') { path } else { m };
        if let Some(name) = path.strip_prefix("block/") {
            return import_block(t, &mut models, pack, &ns, name);
        }
        if let Some(name) = path.strip_prefix("item/") {
            return import_item(t, &mut models, pack, &ns, name);
        }
        if blockstates.iter().any(|(n, p)| *n == ns && p == path)
            || block_models.iter().any(|(n, p)| *n == ns && p == path)
        {
            return import_block(t, &mut models, pack, &ns, path);
        }
        if item_models.iter().any(|(n, p)| *n == ns && p == path) {
            return import_item(t, &mut models, pack, &ns, path);
        }
        return Err(format!("no block or item {ns}:{path} in the pack"));
    }
    if !all {
        let mut list = String::new();
        for (ns, p) in blockstates.iter().chain(&block_models) {
            list.push_str(&format!("\n  {ns}:block/{p}"));
        }
        for (ns, p) in &item_models {
            list.push_str(&format!("\n  {ns}:item/{p}"));
        }
        return Err(format!(
            "pick one with --model <id>, or --all for every block (blockstate) and item. The pack has:{}",
            if list.is_empty() { "\n  (no models)".to_string() } else { list }
        ));
    }
    let mut failed = Vec::new();
    for (ns, name) in &blockstates {
        if let Err(e) = import_block(t, &mut models, pack, ns, name) {
            failed.push(e);
        }
    }
    for (ns, name) in &item_models {
        // A block's item model came with the block.
        if blockstates.iter().any(|(n, p)| n == ns && p == name) {
            continue;
        }
        if let Err(e) = import_item(t, &mut models, pack, ns, name) {
            failed.push(e);
        }
    }
    t.warnings.extend(failed);
    Ok(())
}

// ── Blockbench ───────────────────────────────────────────────────────────────

/// Standard base64 (data URLs of Blockbench textures).
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// `My Texture (1).png` → `my_texture_1`.
fn sanitize(name: &str) -> String {
    let stem = name.strip_suffix(".png").unwrap_or(name);
    let mut s = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('_') {
            s.push('_');
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "texture".into()
    } else {
        s
    }
}

fn import_bbmodel(t: &mut Target, file: &Path, id: Option<&str>, item: bool) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let bb: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let format = bb
        .pointer("/meta/model_format")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !matches!(format, "java_block" | "free" | "") {
        return Err(format!(
            "{}: a {format:?} Blockbench model — only the \"Java Block/Item\" format maps to Minecraft models (File > Convert Project)",
            file.display()
        ));
    }
    if format == "free" {
        t.warnings
            .push("a generic Blockbench model: rotations get snapped to Minecraft's".into());
    }
    let name = match id {
        Some(i) => i.to_string(),
        None => {
            let n = bb
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            sanitize(&n.unwrap_or_else(|| {
                file.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            }))
        }
    };
    let (id_ns, name) = match name.split_once(':') {
        Some((n, p)) => (Some(n.to_string()), p.to_string()),
        None => (None, name),
    };
    if !valid_path(&name) {
        return Err(format!("`{name}` isn't a valid id — pass --id <name>"));
    }
    let ns = id_ns
        .clone()
        .or_else(|| t.ns.clone())
        .ok_or("the mod id isn't a valid namespace — pass --id <namespace>:<name>")?;
    let res_w = bb
        .pointer("/resolution/width")
        .and_then(Value::as_f64)
        .unwrap_or(16.0);
    let res_h = bb
        .pointer("/resolution/height")
        .and_then(Value::as_f64)
        .unwrap_or(16.0);
    let folder = if item { "item" } else { "block" };

    // Textures: slot = index ("0", "1", ...), like Blockbench's export.
    let mut tex = J::obj::<&str>([]);
    let mut uv_scale: Vec<(f64, f64)> = Vec::new();
    let mut particle: Option<String> = None;
    let textures = bb
        .get("textures")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (i, tx) in textures.iter().enumerate() {
        let slot = i.to_string();
        let tex_name = sanitize(tx.get("name").and_then(Value::as_str).unwrap_or("texture"));
        let uw = tx.get("uv_width").and_then(Value::as_f64).unwrap_or(res_w);
        let uh = tx.get("uv_height").and_then(Value::as_f64).unwrap_or(res_h);
        uv_scale.push((16.0 / uw, 16.0 / uh));
        let source = tx.get("source").and_then(Value::as_str).unwrap_or("");
        let value = if let Some(data) = source.strip_prefix("data:image/png;base64,") {
            let png = base64_decode(data).ok_or("a texture's data isn't base64")?;
            let tex_folder = tx
                .get("folder")
                .and_then(Value::as_str)
                .filter(|f| valid_path(f))
                .unwrap_or(folder);
            t.put_texture(&format!("{tex_folder}/{tex_name}"), &png, None)?
        } else {
            let tns = tx
                .get("namespace")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("minecraft");
            let tfolder = tx
                .get("folder")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(folder);
            t.warnings.push(format!("texture {tex_name} isn't embedded in the .bbmodel — referenced as {tns}:{tfolder}/{tex_name}"));
            format!("{tns}:{tfolder}/{tex_name}")
        };
        if tx.get("particle").and_then(Value::as_bool) == Some(true) {
            particle = Some(value.clone());
        }
        tex.set(&slot, J::string(value));
    }
    if let Some(p) = particle {
        tex.set("particle", J::string(p));
    }
    let uuid_slot: BTreeMap<String, usize> = textures
        .iter()
        .enumerate()
        .filter_map(|(i, tx)| {
            tx.get("uuid")
                .and_then(Value::as_str)
                .map(|u| (u.to_string(), i))
        })
        .collect();

    let mut elements = Vec::new();
    for (i, e) in bb
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let what = format!(
            "element {i} ({})",
            e.get("name").and_then(Value::as_str).unwrap_or("?")
        );
        if e.get("type")
            .and_then(Value::as_str)
            .is_some_and(|ty| ty != "cube")
        {
            t.warnings.push(format!("{what}: not a cube — skipped"));
            continue;
        }
        if e.get("visibility").and_then(Value::as_bool) == Some(false) {
            t.warnings
                .push(format!("{what}: hidden in Blockbench — imported anyway"));
        }
        let vec3 = |t: &mut Target, k: &str| -> Vec<f64> {
            let a: Vec<f64> = e
                .get(k)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            a.into_iter().map(|n| clamp_coord(t, &what, n)).collect()
        };
        let mut el = J::obj::<&str>([]);
        if let Some(n) = e.get("name").and_then(Value::as_str) {
            el.set("name", J::string(n));
        }
        // Blockbench's inflate grows the box both ways.
        let inflate = e.get("inflate").and_then(Value::as_f64).unwrap_or(0.0);
        let mut from = vec3(t, "from");
        let mut to = vec3(t, "to");
        if inflate != 0.0 {
            for v in &mut from {
                *v -= inflate;
            }
            for v in &mut to {
                *v += inflate;
            }
        }
        el.set("from", J::nums(&from));
        el.set("to", J::nums(&to));
        let rot: Vec<f64> = e
            .get("rotation")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        let nonzero: Vec<(usize, f64)> = rot
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, a)| *a != 0.0)
            .collect();
        if nonzero.len() > 1 {
            t.warnings.push(format!("{what}: rotated on more than one axis — only the first kept (Minecraft rotates elements on one axis)"));
        }
        if let Some((axis, angle)) = nonzero.first().copied() {
            let angle = snap_angle(t, &what, angle);
            if angle != 0.0 {
                let mut r = J::obj([("axis", J::string(["x", "y", "z"][axis.min(2)]))]);
                r.set("angle", J::number(angle));
                r.set("origin", J::nums(&vec3(t, "origin")));
                if e.get("rescale").and_then(Value::as_bool) == Some(true) {
                    r.set("rescale", J::of(V::Bool(true)));
                }
                el.set("rotation", r);
            }
        }
        if e.get("shade").and_then(Value::as_bool) == Some(false) {
            el.set("shade", J::of(V::Bool(false)));
        }
        let mut faces = J::obj::<&str>([]);
        for dir in super::DIRECTIONS {
            let Some(f) = e.pointer(&format!("/faces/{dir}")) else {
                continue;
            };
            let index = match f.get("texture") {
                Some(Value::Number(n)) => n.as_u64().map(|n| n as usize),
                Some(Value::String(u)) => uuid_slot.get(u).copied(),
                _ => None,
            };
            let Some(index) = index.filter(|i| *i < textures.len()) else {
                continue; // No texture: Blockbench doesn't export the face.
            };
            let (su, sv) = uv_scale[index];
            let mut fo = J::obj([("texture", J::string(format!("#{index}")))]);
            if let Some(uv) = f.get("uv").and_then(Value::as_array) {
                let uv: Vec<f64> = uv
                    .iter()
                    .filter_map(Value::as_f64)
                    .enumerate()
                    .map(|(k, n)| {
                        ((n * if k % 2 == 0 { su } else { sv }).clamp(0.0, 16.0) * 10000.0).round()
                            / 10000.0
                    })
                    .collect();
                fo.set("uv", J::nums(&uv));
            }
            if let Some(r) = f
                .get("rotation")
                .and_then(Value::as_f64)
                .filter(|r| *r != 0.0)
            {
                fo.set("rotation", J::number(r));
            }
            if let Some(c) = f
                .get("cullface")
                .and_then(Value::as_str)
                .filter(|c| !c.is_empty())
            {
                fo.set("cullface", J::string(c));
            }
            if let Some(n) = f.get("tint").and_then(Value::as_f64).filter(|n| *n >= 0.0) {
                fo.set("tint", J::number(n));
            }
            faces.set(dir, fo);
        }
        el.set("faces", faces);
        elements.push(el);
    }

    let mut model = J::obj::<&str>([]);
    let parent = bb
        .get("parent")
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty());
    if let Some(p) = parent {
        match template_of(p) {
            Some(tpl) if elements.is_empty() => model.set("parent", J::string(tpl.name)),
            _ if elements.is_empty() => model.set("parent", J::string(full_id(p))),
            _ => {}
        }
    } else if elements.is_empty() {
        return Err("the model has no cubes".into());
    }
    if !tex.members().is_empty() {
        model.set("textures", tex);
    }
    if !elements.is_empty() {
        model.set("elements", J::of(V::Arr(elements)));
    }
    if let Some(Value::Object(d)) = bb.get("display") {
        let mut display = J::obj::<&str>([]);
        for (slot, v) in d {
            if !super::DISPLAY_SLOTS.contains(&slot.as_str()) {
                continue;
            }
            let mut o = J::obj::<&str>([]);
            for k in ["rotation", "translation", "scale"] {
                if let Some(a) = v.get(k).and_then(Value::as_array) {
                    let a: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
                    let default = if k == "scale" { [1.0; 3] } else { [0.0; 3] };
                    if a.len() == 3 && a[..] != default[..] {
                        o.set(k, J::nums(&a));
                    }
                }
            }
            if !o.members().is_empty() {
                display.set(slot, o);
            }
        }
        if !display.members().is_empty() {
            model.set("display", display);
        }
    }
    if bb.get("ambientocclusion").and_then(Value::as_bool) == Some(false) {
        model.set("ambient_occlusion", J::of(V::Bool(false)));
    }
    if bb.get("front_gui_light").and_then(Value::as_bool) == Some(true) {
        model.set("gui_light", J::string("front"));
    }

    let mut j = J::obj::<&str>([]);
    if t.ns.is_none() || id_ns.is_some() {
        j.set("id", J::string(format!("{ns}:{name}")));
    }
    j.set("model", model);
    let leaf = name.rsplit('/').next().unwrap_or(&name).to_string();
    let rel = if item {
        format!("content/items/{leaf}.yitem")
    } else {
        format!("content/blocks/{leaf}.yblock")
    };
    t.write_file(&rel, to_pretty(&j).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("yog-import-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn target(root: &Path) -> Target {
        Target {
            root: root.to_path_buf(),
            ns: Some("mymod".into()),
            force: false,
            written: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn decodes_base64() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
    }

    #[test]
    fn imports_a_pack_block_and_compiles_it() {
        let pack_dir = temp("pack");
        let w = |rel: &str, s: &[u8]| {
            let p = pack_dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, s).unwrap();
        };
        w(
            "assets/foo/blockstates/lamp.json",
            br#"{"variants":{"":[{"model":"foo:block/lamp"},{"model":"foo:block/lamp","y":90}]}}"#,
        );
        w("assets/foo/models/block/lamp_base.json", br##"{"textures":{"0":"foo:block/lamp","particle":"#0"},"render_type":"cutout",
            "elements":[{"from":[4,0,4],"to":[12,8,12],"rotation":{"angle":20,"axis":"y","origin":[8,8,8]},
            "faces":{"north":{"uv":[0,0,8,8],"texture":"#0","cullface":"north"}}}]}"##);
        w(
            "assets/foo/models/block/lamp.json",
            br#"{"parent":"foo:block/lamp_base"}"#,
        );
        w(
            "assets/foo/models/item/lamp.json",
            br#"{"parent":"foo:block/lamp"}"#,
        );
        w("assets/foo/textures/block/lamp.png", b"PNG");
        w(
            "assets/foo/lang/en_us.json",
            br#"{"block.foo.lamp":"Lamp"}"#,
        );
        let root = temp("mod");
        let mut t = target(&root);
        let pack = Pack::open(&pack_dir).unwrap();
        import_pack(&mut t, &pack, Some("foo:block/lamp"), false).unwrap();
        assert!(
            t.warnings.iter().any(|w| w.contains("20° → 22.5°")),
            "{:?}",
            t.warnings
        );
        let text = std::fs::read_to_string(root.join("content/blocks/lamp.yblock")).unwrap();
        assert!(text.contains("\"name\": \"Lamp\""), "{text}");
        assert!(text.contains("\"render_layer\": \"cutout\""), "{text}");
        assert!(text.contains("\"0\": \"block/lamp.png\""), "{text}");
        assert!(!text.contains("item_model"), "{text}");
        assert_eq!(
            std::fs::read(root.join("resources/block/lamp.png")).unwrap(),
            b"PNG"
        );
        let c = super::super::compile(&root, "mymod", &crate::yres::Refs::default()).unwrap();
        let bs = c
            .outputs
            .iter()
            .find(|(p, _)| p == "assets/mymod/blockstates/lamp.json")
            .unwrap();
        let bs: Value = serde_json::from_slice(&bs.1).unwrap();
        assert_eq!(bs["variants"][""][1]["y"], serde_json::json!(90));
        let _ = std::fs::remove_dir_all(pack_dir);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn imports_a_bbmodel() {
        let root = temp("bb");
        let bb = root.join("thing.bbmodel");
        std::fs::write(
            &bb,
            r#"{"meta":{"model_format":"java_block"},"name":"Thing","resolution":{"width":32,"height":32},
                "textures":[{"name":"thing.png","uuid":"u1","source":"data:image/png;base64,aGVsbG8="}],
                "elements":[{"name":"cube","from":[0,0,0],"to":[16,8,16],"origin":[8,8,8],"rotation":[0,45,0],
                  "faces":{"north":{"uv":[0,0,32,16],"texture":0},"up":{"uv":[0,0,32,32],"texture":null}}}]}"#,
        )
        .unwrap();
        let mut t = target(&root);
        import_bbmodel(&mut t, &bb, None, false).unwrap();
        let text = std::fs::read_to_string(root.join("content/blocks/thing.yblock")).unwrap();
        assert!(text.contains("\"uv\": [0, 0, 16, 8]"), "{text}");
        assert!(!text.contains("\"up\""), "{text}");
        assert_eq!(
            std::fs::read(root.join("resources/block/thing.png")).unwrap(),
            b"hello"
        );
        super::super::compile(&root, "mymod", &crate::yres::Refs::default()).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
