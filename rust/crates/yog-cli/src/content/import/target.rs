//! Where imports go: the mod's root and namespace, the files written,
//! what to warn of.

use std::path::PathBuf;

use yog_data::Schema;
use yog_ids::Format;

use crate::yres::RESOURCES_DIR;

/// Where imports go.
pub(super) struct Target {
    pub(super) root: PathBuf,
    /// The mod's namespace, when its id is one.
    pub(super) ns: Option<String>,
    pub(super) force: bool,
    pub(super) written: Vec<String>,
    pub(super) warnings: Vec<String>,
}

impl Target {
    /// The `"id"` to write for `name` from namespace `from_ns`: none when
    /// the mod's namespace applies.
    pub(super) fn id_for(&self, from_ns: &str, name: &str) -> Option<String> {
        self.ns.is_none().then(|| yog_ids::join(from_ns, name))
    }

    /// The namespace textures bundled for an entry of `from_ns` land in.
    pub(super) fn texture_ns(&self, from_ns: &str) -> String {
        self.ns.clone().unwrap_or_else(|| from_ns.to_string())
    }

    pub(super) fn write_file(&mut self, rel: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.root.join(rel);
        // A Yog format's file is kept binary.
        let binary;
        let bytes = match yog_ids::Format::of_path(&path).filter(|f| f.is_data()) {
            Some(_) if !yog_data::source::is_binary(bytes) => {
                let text = std::str::from_utf8(bytes).map_err(|e| format!("{rel}: {e}"))?;
                binary = yog_data::source::binary(text).map_err(|e| format!("{rel}: {e}"))?;
                binary.as_slice()
            }
            _ => bytes,
        };
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

    /// Puts a texture's picture into its `.yres` set (as bytes, named by
    /// its path: `block.ruby` in `BlockImages.yres`) and writes its `<path>.ytexture`
    /// (its atlas by its folder, the animation an `.mcmeta` gave) → its
    /// value in a content file (`@texture:<ns>:<path>`).
    pub(super) fn put_texture(
        &mut self,
        ns: &str,
        path: &str,
        png: &[u8],
        mcmeta: Option<&[u8]>,
    ) -> Result<String, String> {
        let name = path.replace(yog_ids::sep::PATH, yog_ids::sep::DOT);
        let set = yog_formats::texture::picture_set(path);
        let set_rel = format!("{RESOURCES_DIR}/{}", Format::Res.file(&set));
        let set_file = self.root.join(&set_rel);
        let mut entries = match crate::files::read_source(&set_file) {
            Ok(text) => crate::yres::parse(&text).map_err(|e| format!("{set_rel}: {e}"))?,
            Err(_) => Vec::new(),
        };
        let picture = crate::yres::Value::Bytes(png.to_vec());
        match entries.iter_mut().find(|e| e.name == name) {
            Some(e) if e.value == picture => {}
            // A picture already there and different: keep it, warn.
            Some(_) if !self.force => self.warnings.push(format!(
                "{set_rel}: {name} exists and differs — kept it (--force overwrites)"
            )),
            Some(e) => e.value = picture,
            None => entries.push(crate::yres::Entry::picture(&name, png.to_vec())),
        }
        if let Some(dir) = set_file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{set_rel}: {e}"))?;
        }
        crate::files::write_source(&set_file, &crate::yres::write(&entries))
            .map_err(|e| format!("{set_rel}: {e}"))?;
        if !self.written.contains(&set_rel) {
            self.written.push(set_rel);
        }
        let rel = format!("{RESOURCES_DIR}/{}", Format::Texture.file(path));
        if !self.root.join(&rel).is_file() || self.force {
            let texture = yog_formats::texture::TextureFile {
                image: yog_data::Res { set, key: name },
                atlas: yog_registry::Atlas::of_path(path).map(|a| a.id.clone()),
                animation: mcmeta
                    .and_then(|m| serde_json::from_slice(m).ok())
                    .and_then(|m| yog_formats::texture::Animation::from_mcmeta(&m)),
                blur: false,
                clamp: false,
            };
            self.write_file(
                &rel,
                yog_data::text::to_string_pretty(&texture.write()).as_bytes(),
            )?;
        }
        Ok(format!(
            "{}{}",
            yog_ids::reference::TEXTURE,
            yog_ids::join(ns, path)
        ))
    }
}
