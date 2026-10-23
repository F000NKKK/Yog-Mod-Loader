//! A mod's media as the tools see them: its `.yres` pictures (bytes and
//! all), its `.ytexture`s, and the files the build makes of them — so the
//! IDE previews and edits what the build will make, converting nothing
//! itself. Reading is cached until a file changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use yog_sync::{Arc, Mutex, OnceLock};

use yog_data::Res;
use yog_formats::texture::{Animation, TextureFile};
use yog_formats::yres::{Entry, Kind, Value, split_file_name};
use yog_ids::Format;

/// One picture of a `.yres` set.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Picture {
    /// Its set (`Images` for `Images.yres`) and name: `@Images.gui.logo`.
    pub(crate) set: String,
    pub(crate) name: String,
    /// The `.yres` file it's in.
    pub(crate) file: PathBuf,
    /// The picture, as stored.
    pub(crate) bytes: Arc<Vec<u8>>,
}

impl Picture {
    /// Its path under the mod's textures, as the build writes it: `gui.logo` → `gui/logo`.
    pub(crate) fn path(&self) -> String {
        self.name.replace('.', yog_ids::sep::PATH)
    }
}

/// One of the mod's textures.
#[derive(Debug, Clone)]
pub(crate) struct Texture {
    pub(crate) id: String,
    /// Its `.ytexture`.
    pub(crate) file: PathBuf,
    pub(crate) texture: TextureFile,
}

impl Texture {
    /// Its picture, when its `.yres` has it.
    pub(crate) fn picture(&self, root: &Path) -> Option<Picture> {
        picture(root, &self.texture.image.set, &self.texture.image.key)
    }
}

/// The id of the mod at `root` (its manifest's).
pub(crate) fn mod_id(root: &Path) -> Option<String> {
    crate::manifest::YogToml::read(&root.join(yog_ids::file::MANIFEST))
        .ok()
        .map(|m| m.id)
}

type Cache = Mutex<HashMap<PathBuf, (SystemTime, Arc<Vec<Picture>>)>>;
static PICTURES: OnceLock<Cache> = OnceLock::new();

/// The pictures of the `.yres` file at `file` (a set's neutral one).
fn pictures_of(file: &Path, set: &str) -> Arc<Vec<Picture>> {
    let modified = std::fs::metadata(file)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let cache = PICTURES.get_or_init(Default::default);
    if let Some((at, pictures)) = cache.lock().get(file).cloned()
        && at == modified
    {
        return pictures;
    }
    let entries = crate::files::read_source(file)
        .ok()
        .and_then(|t| yog_formats::yres::parse(&t).ok())
        .unwrap_or_default();
    let pictures: Arc<Vec<Picture>> = Arc::new(
        entries
            .into_iter()
            .filter_map(|e| match (e.kind, e.value) {
                (Kind::Image, Value::Bytes(b)) => Some(Picture {
                    set: set.to_string(),
                    name: e.name,
                    file: file.to_path_buf(),
                    bytes: Arc::new(b),
                }),
                _ => None,
            })
            .collect(),
    );
    let mut c = cache.lock();
    c.insert(file.to_path_buf(), (modified, pictures.clone()));
    pictures
}

/// Every picture of the mod at `root`.
pub(crate) fn pictures(root: &Path) -> Vec<Picture> {
    let mut out = Vec::new();
    for file in crate::files::with_ext(&root.join(yog_ids::project::RESOURCES), Format::Res.ext()) {
        // A culture variant (`Images.ru_ru.yres`) has no pictures of its own.
        let Some((set, None)) = file
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(split_file_name)
        else {
            continue;
        };
        out.extend(pictures_of(&file, set).iter().cloned());
    }
    out
}

/// Picture `@<set>.<name>` of the mod at `root`.
pub(crate) fn picture(root: &Path, set: &str, name: &str) -> Option<Picture> {
    let file = root
        .join(yog_ids::project::RESOURCES)
        .join(Format::Res.file(set));
    pictures_of(&file, set)
        .iter()
        .find(|p| p.name == name)
        .cloned()
}

/// Sound `@<set>.<name>` of the mod at `root`, as stored (any format the build converts).
pub(crate) fn sound(root: &Path, set: &str, name: &str) -> Option<Vec<u8>> {
    let file = root
        .join(yog_ids::project::RESOURCES)
        .join(Format::Res.file(set));
    let text = crate::files::read_source(&file).ok()?;
    yog_formats::yres::parse(&text)
        .ok()?
        .into_iter()
        .find(|e| e.name == name && e.kind == Kind::Sound)
        .and_then(|e| match e.value {
            Value::Bytes(b) => Some(b),
            _ => None,
        })
}

/// The game's sound file `id` (`minecraft:dig/sand1`, no namespace: the
/// game's), from its assets on this machine.
pub(crate) fn game_sound(id: &str) -> Option<Vec<u8>> {
    let (ns, path) = yog_ids::split(id);
    let key = format!(
        "{ns}{}{}{path}{}",
        yog_ids::sep::PATH,
        yog_ids::res::SOUNDS,
        yog_ids::res::OGG
    );
    crate::design::assets::vanilla(&[]).and_then(|v| v.read_all(&key).pop())
}

/// Writes `bytes` as picture `@<set>.<name>` of the mod at `root` (the
/// set's file made when there's none); the file written.
pub(crate) fn save_picture(
    root: &Path,
    set: &str,
    name: &str,
    bytes: Vec<u8>,
) -> Result<PathBuf, String> {
    let file = root
        .join(yog_ids::project::RESOURCES)
        .join(Format::Res.file(set));
    let mut entries = match crate::files::read_source(&file) {
        Ok(text) => {
            yog_formats::yres::parse(&text).map_err(|e| format!("{}: {e}", file.display()))?
        }
        Err(_) => Vec::new(),
    };
    match entries.iter_mut().find(|e| e.name == name) {
        Some(e) => e.value = Value::Bytes(bytes),
        None => entries.push(Entry::picture(name, bytes)),
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    crate::files::write_source(&file, &yog_formats::yres::write(&entries))?;
    Ok(file)
}

/// Where texture `id` of the mod at `root` is described, whether it is yet or not.
fn texture_file(root: &Path, id: &str) -> Option<PathBuf> {
    let (ns, path) = yog_ids::split(id);
    (mod_id(root)? == ns).then(|| {
        root.join(yog_ids::project::RESOURCES)
            .join(Format::Texture.file(path))
    })
}

/// Texture `id` of the mod at `root`, when it has a `.ytexture`.
pub(crate) fn texture(root: &Path, id: &str) -> Option<Texture> {
    let file = texture_file(root, id)?;
    let texture = yog_data::from_text::<TextureFile>(&crate::files::read_source(&file).ok()?)
        .ok()?
        .value;
    Some(Texture {
        id: yog_ids::qualify(id),
        file,
        texture,
    })
}

/// Every texture of the mod at `root`.
pub(crate) fn textures(root: &Path) -> Vec<Texture> {
    let Some(ns) = mod_id(root) else {
        return Vec::new();
    };
    let resources = root.join(yog_ids::project::RESOURCES);
    crate::files::with_ext(&resources, Format::Texture.ext())
        .into_iter()
        .filter_map(|f| {
            let rel = f
                .strip_prefix(&resources)
                .ok()?
                .to_string_lossy()
                .replace('\\', yog_ids::sep::PATH);
            texture(root, &yog_formats::texture::id_of(&ns, &rel)?)
        })
        .collect()
}

/// Whether texture `id` is the mod's own (it can be painted in place, or added).
pub(crate) fn is_own_texture(root: &Path, id: &str) -> bool {
    texture_file(root, id).is_some()
}

/// Writes `png` as texture `id`'s picture: into the `.yres` picture its
/// `.ytexture` names, or — a new texture — into the set its folder's
/// pictures go in, with a `.ytexture` naming it. The `.yres` written.
pub(crate) fn save_texture_picture(root: &Path, id: &str, png: Vec<u8>) -> Result<PathBuf, String> {
    let file = texture_file(root, id).ok_or_else(|| format!("{id} isn't a texture of this mod"))?;
    if let Some(own) = texture(root, id) {
        return save_picture(root, &own.texture.image.set, &own.texture.image.key, png);
    }
    let path = yog_ids::split(id).1;
    let image = Res {
        set: yog_formats::texture::picture_set(path),
        key: path.replace(yog_ids::sep::PATH, "."),
    };
    let written = save_picture(root, &image.set, &image.key, png)?;
    let texture = TextureFile {
        image,
        atlas: yog_registry::Atlas::of_path(path).map(|a| a.id.clone()),
        animation: None,
        blur: false,
        clamp: false,
    };
    write_texture(&file, &texture)?;
    Ok(written)
}

/// Sets texture `id`'s animation (`None`: still); the `.ytexture` written.
pub(crate) fn save_texture_animation(
    root: &Path,
    id: &str,
    animation: Option<Animation>,
) -> Result<PathBuf, String> {
    let mut own = texture(root, id)
        .ok_or_else(|| format!("{id} has no .{} in this mod", Format::Texture.ext()))?;
    own.texture.animation = animation;
    write_texture(&own.file, &own.texture)?;
    Ok(own.file)
}

fn write_texture(file: &Path, texture: &TextureFile) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(file, yog_data::to_bytes(texture))
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// The file the build makes at `assets/<ns>/<path>` from the mod at `root`
/// — a texture's PNG or `.png.mcmeta` (a `.ytexture`'s picture and
/// animation, else a `.yres` picture of that path) — `None` when the build
/// makes none there.
pub(crate) fn built_asset(root: &Path, ns: &str, path: &str) -> Option<Vec<u8>> {
    if mod_id(root)? != ns {
        return None;
    }
    let file = path.strip_prefix(yog_ids::res::TEXTURES)?;
    if let Some(stem) = file
        .strip_suffix(yog_ids::res::MCMETA)
        .and_then(|f| f.strip_suffix(yog_ids::res::PNG))
    {
        let meta = texture(root, &yog_ids::join(ns, stem))?
            .texture
            .game_mcmeta()?;
        return serde_json::to_vec(&meta).ok();
    }
    let stem = file.strip_suffix(yog_ids::res::PNG)?;
    let picture = match texture(root, &yog_ids::join(ns, stem)) {
        Some(own) => own.picture(root)?,
        None => pictures(root).into_iter().find(|p| p.path() == stem)?,
    };
    // The game takes PNG; a picture stored as anything else is converted, as the build does.
    let image = yog_media::image::decode(&picture.bytes).ok()?;
    yog_media::image::encode_png(&image).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        yog_media::image::encode_png(&yog_media::image::Image {
            width: 1,
            height: 1,
            rgba: vec![9, 9, 9, 255],
        })
        .unwrap()
    }

    #[test]
    fn a_painted_texture_becomes_a_picture_and_a_ytexture() {
        let dir = std::env::temp_dir().join(format!("yog-media-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(yog_ids::file::MANIFEST), "[mod]\nid = \"mymod\"\n").unwrap();
        save_texture_picture(&dir, "mymod:block/ruby", png()).unwrap();
        let own = texture(&dir, "mymod:block/ruby").unwrap();
        assert_eq!(own.texture.atlas.as_deref(), Some("minecraft:blocks"));
        assert_eq!(
            own.picture(&dir).unwrap().bytes.as_slice(),
            png().as_slice()
        );
        assert_eq!(
            built_asset(&dir, "mymod", "textures/block/ruby.png"),
            Some(png())
        );
        save_texture_animation(
            &dir,
            "mymod:block/ruby",
            Some(Animation {
                frame_time: Some(4),
                ..Animation::default()
            }),
        )
        .unwrap();
        assert!(built_asset(&dir, "mymod", "textures/block/ruby.png.mcmeta").is_some());
        assert_eq!(textures(&dir).len(), 1);
        assert!(!is_own_texture(&dir, "minecraft:block/stone"));
        assert!(pictures(&dir).iter().any(|p| p.path() == "block/ruby"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
