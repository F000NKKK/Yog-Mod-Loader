//! Minecraft's own resources, read from the user's local game files — the
//! client jar a Gradle dev setup (Loom, ForgeGradle, NeoGradle) or the
//! official launcher already downloaded, plus its asset index/objects —
//! so the designer draws with the game's font and textures. Nothing is
//! bundled or redistributed: when no client jar is found the designer says
//! so and draws with what it has (flat rects, a fallback glyph box).
//!
//! Resources resolve like a resource-pack stack: the mod's own images
//! (`.yres`) first, then the client jar, then the asset objects (where the
//! unifont lives).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use yog_sync::{Arc, Mutex, OnceLock};

pub(crate) use yog_graphics::Image;

/// Minecraft's missing texture: 16×16, magenta/black quarters.
pub(crate) fn missing_image() -> Image {
    let mut image = Image::new(16, 16);
    for y in 0..16 {
        for x in 0..16 {
            image.set_pixel(
                x,
                y,
                if (x < 8) == (y < 8) {
                    yog_graphics::Color::from_rgba8([0xF8, 0, 0xF8, 0xFF])
                } else {
                    yog_graphics::Color::from_rgba8([0, 0, 0, 0xFF])
                },
            );
        }
    }
    image
}

/// A picture in any format the build reads (`yog_media`), as RGBA.
pub(crate) fn decode_png(bytes: &[u8]) -> Option<Image> {
    yog_media::image::decode(bytes).ok()
}

/// Where the vanilla resources come from.
pub(crate) struct Vanilla {
    pub(crate) jar: PathBuf,
    pub(crate) version: String,
    zip: Mutex<zip::ZipArchive<File>>,
    /// Asset objects dir + `key → hash` from the version's index.
    objects: Option<(PathBuf, HashMap<String, String>)>,
}

impl Vanilla {
    /// A client jar at `jar` (tests stand one in for the game's).
    #[cfg(test)]
    pub(crate) fn from_jar(jar: PathBuf, version: &str) -> Option<Vanilla> {
        let zip = zip::ZipArchive::new(File::open(&jar).ok()?).ok()?;
        Some(Vanilla {
            jar,
            version: version.to_string(),
            zip: Mutex::new(zip),
            objects: None,
        })
    }

    /// `key` from the jar and from the asset objects — both, for files the
    /// download overrides (the jar's `font/include/unifont.json` is an
    /// empty stub; the real one is an asset object).
    pub(crate) fn read_all(&self, key: &str) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut zip = self.zip.lock();
        if let Ok(mut entry) = zip.by_name(&format!("assets/{key}")) {
            let mut bytes = Vec::new();
            if entry.read_to_end(&mut bytes).is_ok() {
                out.push(bytes);
            }
        }
        if let Some((dir, index)) = &self.objects
            && let Some(bytes) = index
                .get(key)
                .and_then(|hash| std::fs::read(dir.join(&hash[..2]).join(hash)).ok())
        {
            out.push(bytes);
        }
        out
    }

    /// `data/<key>` from the jar (tags, recipes, …), if there.
    pub(crate) fn read_data(&self, key: &str) -> Option<Vec<u8>> {
        let mut zip = self.zip.lock();
        let mut entry = zip.by_name(&format!("data/{key}")).ok()?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    /// The jar's file names under `prefix` (`data/minecraft/loot_tables/`), the prefix cut.
    pub(crate) fn names_under(&self, prefix: &str) -> Vec<String> {
        let zip = self.zip.lock();
        zip.file_names()
            .filter_map(|n| Some(n.ok()?.strip_prefix(prefix)?.to_string()))
            .filter(|n| !n.is_empty() && !n.ends_with('/'))
            .collect()
    }

    /// The game's item and block texture names (`apple`, `stone`, ...),
    /// sorted — what `item:` / `block:` sources can show in the designer.
    pub(crate) fn texture_names(&self) -> (Vec<String>, Vec<String>) {
        let (mut items, mut blocks) = (Vec::new(), Vec::new());
        let zip = self.zip.lock();
        for name in zip.file_names() {
            let Ok(name) = name else { continue };
            let Some(stem) = name
                .strip_prefix("assets/minecraft/textures/")
                .and_then(|n| n.strip_suffix(yog_ids::res::PNG))
            else {
                continue;
            };
            if let Some(item) = stem.strip_prefix("item/") {
                items.push(item.to_string());
            } else if let Some(block) = stem.strip_prefix("block/") {
                blocks.push(block.to_string());
            }
        }
        items.sort();
        blocks.sort();
        (items, blocks)
    }

    fn read(&self, key: &str) -> Option<Vec<u8>> {
        // `key` is `namespace/path` — `assets/<key>` in the jar.
        let mut zip = self.zip.lock();
        if let Ok(mut entry) = zip.by_name(&format!("assets/{key}")) {
            let mut out = Vec::new();
            if entry.read_to_end(&mut out).is_ok() {
                return Some(out);
            }
        }
        let (dir, index) = self.objects.as_ref()?;
        let hash = index.get(key)?;
        std::fs::read(dir.join(&hash[..2]).join(hash)).ok()
    }
}

fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

/// `1.21.1` > `1.20.1` > `1.9`.
fn version_key(v: &str) -> Vec<u32> {
    v.split(['.', '-'])
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// Every client jar on this machine, as `(version, path)`.
fn client_jars(extra_roots: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var(yog_ids::env::MC_CLIENT_JAR) {
        let p = PathBuf::from(p);
        if p.is_file() {
            out.push(("999".to_string(), p));
        }
    }
    let mut version_dirs: Vec<(PathBuf, &str)> = Vec::new();
    if let Some(h) = home() {
        let caches = h.join(".gradle/caches");
        version_dirs.push((caches.join("fabric-loom"), "minecraft-client.jar"));
        version_dirs.push((
            caches.join("forge_gradle/minecraft_repo/versions"),
            "client.jar",
        ));
        version_dirs.push((caches.join("minecraft/versions"), "client.jar"));
        version_dirs.push((h.join(".minecraft/versions"), ""));
        version_dirs.push((h.join("Library/Application Support/minecraft/versions"), ""));
    }
    if let Some(d) = dirs::data_dir() {
        version_dirs.push((d.join(".minecraft/versions"), ""));
    }
    for root in extra_roots {
        // A Yog workspace: the loader's NeoGradle caches beside the mod.
        for loader in [
            yog_ids::loader::NEOFORGE,
            yog_ids::loader::FABRIC,
            yog_ids::loader::FORGE,
        ] {
            for base in [root.join("Yog-Mod-Loader"), root.join("../Yog-Mod-Loader")] {
                version_dirs.push((
                    base.join(loader).join(".gradle/caches/minecraft/versions"),
                    "client.jar",
                ));
            }
        }
    }
    for (dir, file) in version_dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let version = e.file_name().to_string_lossy().into_owned();
            if !version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }
            let jar = if file.is_empty() {
                e.path().join(format!("{version}.jar"))
            } else {
                e.path().join(file)
            };
            if jar.is_file() {
                out.push((version, jar));
            }
        }
    }
    out
}

/// The newest client jar on this machine (`YOG_MC_CLIENT_JAR` first).
pub(crate) fn newest_client_jar(extra_roots: &[PathBuf]) -> Option<PathBuf> {
    client_jars(extra_roots)
        .into_iter()
        .max_by_key(|(v, _)| version_key(v))
        .map(|(_, p)| p)
}

/// The asset objects dir and the index for `version` (any index that knows
/// the unifont when there's no exact match).
fn asset_objects(version: &str) -> Option<(PathBuf, HashMap<String, String>)> {
    let h = home()?;
    let mut dirs = vec![
        h.join(".gradle/caches/fabric-loom/assets"),
        h.join(".gradle/caches/forge_gradle/assets"),
        h.join(".gradle/caches/minecraft/assets"),
        h.join(".minecraft/assets"),
    ];
    if let Some(d) = dirs::data_dir() {
        dirs.push(d.join(".minecraft/assets"));
    }
    let mut best: Option<(bool, PathBuf, HashMap<String, String>)> = None;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir.join("indexes")) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let exact =
                name.starts_with(&format!("{version}-")) || name == format!("{version}.json");
            if best.as_ref().is_some_and(|b| b.0 || !exact) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let Some(objects) = json.get("objects").and_then(|o| o.as_object()) else {
                continue;
            };
            if !objects.contains_key("minecraft/font/include/unifont.json") {
                continue;
            }
            let index = objects
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.get("hash")?.as_str()?.to_string())))
                .collect();
            best = Some((exact, dir.join("objects"), index));
        }
    }
    best.map(|(_, dir, index)| (dir, index))
}

static VANILLA: OnceLock<Mutex<Option<Arc<Vanilla>>>> = OnceLock::new();

/// The newest client jar found (cached after the first successful lookup).
pub(crate) fn vanilla(extra_roots: &[PathBuf]) -> Option<Arc<Vanilla>> {
    let cell = VANILLA.get_or_init(|| Mutex::new(None));
    let mut slot = cell.lock();
    if let Some(v) = slot.as_ref() {
        return Some(v.clone());
    }
    let mut jars = client_jars(extra_roots);
    jars.sort_by_key(|(v, _)| std::cmp::Reverse(version_key(v)));
    for (version, jar) in jars {
        let Ok(file) = File::open(&jar) else { continue };
        let Ok(zip) = zip::ZipArchive::new(file) else {
            continue;
        };
        let v = Arc::new(Vanilla {
            objects: asset_objects(&version),
            jar,
            version,
            zip: Mutex::new(zip),
        });
        *slot = Some(v.clone());
        return Some(v);
    }
    None
}

static ALL_VANILLA: OnceLock<Vec<Arc<Vanilla>>> = OnceLock::new();

/// Every Minecraft version found, newest first — for what one version has
/// and another hasn't (a tag removed in 1.21).
pub(crate) fn vanilla_versions(extra_roots: &[PathBuf]) -> &'static [Arc<Vanilla>] {
    ALL_VANILLA.get_or_init(|| {
        let mut jars = client_jars(extra_roots);
        jars.sort_by_key(|(v, _)| std::cmp::Reverse(version_key(v)));
        jars.dedup_by(|a, b| a.0 == b.0);
        jars.into_iter()
            .filter_map(|(version, jar)| {
                let zip = zip::ZipArchive::new(File::open(&jar).ok()?).ok()?;
                Some(Arc::new(Vanilla {
                    objects: asset_objects(&version),
                    jar,
                    version,
                    zip: Mutex::new(zip),
                }))
            })
            .collect()
    })
}

/// Resolves `namespace:path` resources for one mod: its `.yres` images, then the game's.
#[derive(Clone)]
pub(crate) struct Resources {
    pub(crate) mod_root: Option<PathBuf>,
    pub(crate) vanilla: Option<Arc<Vanilla>>,
}

impl Resources {
    pub(crate) fn for_mod(mod_root: Option<&Path>) -> Resources {
        let roots: Vec<PathBuf> = mod_root.into_iter().map(Path::to_path_buf).collect();
        let parents: Vec<PathBuf> = roots
            .iter()
            .filter_map(|r| r.parent().map(Path::to_path_buf))
            .collect();
        Resources {
            mod_root: mod_root.map(Path::to_path_buf),
            vanilla: vanilla(&[roots, parents].concat()),
        }
    }

    /// The bytes of resource `<ns>:<path>` (`textures/gui/x.png`): the mod's
    /// own are its `.yres` images (a mod has no `assets/`), the rest the game's.
    pub(crate) fn read(&self, ns: &str, path: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self
            .mod_root
            .as_deref()
            .and_then(|root| crate::media::built_asset(root, ns, path))
        {
            return Some(bytes);
        }
        // Yog's own assets, as every host ships them.
        if ns == yog_ids::ns::YOG
            && let Some(bytes) = yog_gfx::image::own_asset(path)
        {
            return Some(bytes.to_vec());
        }
        self.vanilla.as_ref()?.read(&format!("{ns}/{path}"))
    }

    /// A texture by full resource id (`minecraft:textures/item/apple.png`).
    pub(crate) fn texture(&self, id: &str) -> Option<Arc<Image>> {
        type Cache = Mutex<HashMap<(Option<PathBuf>, String), Option<Arc<Image>>>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let key = (self.mod_root.clone(), id.to_string());
        if let Some(hit) = cache.lock().get(&key) {
            return hit.clone();
        }
        let (ns, path) = yog_ids::split(id);
        let image = self
            .read(ns, path)
            .and_then(|b| decode_png(&b))
            .map(Arc::new);
        let mut c = cache.lock();
        c.insert(key, image.clone());
        image
    }
}
