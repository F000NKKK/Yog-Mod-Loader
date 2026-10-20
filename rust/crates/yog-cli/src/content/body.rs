//! `.yentity`, `.ymodel`/`.yanim` and `.yfont` files: checked and built.

use yog_ids::Format;
use std::path::Path;
use crate::content::BodyFile;
use crate::content::CONTENT_DIR;
use crate::content::Compiled;
use crate::content::Ctx;
use crate::content::EntityFile;
use crate::content::FontFile;
use crate::content::block_item::text_key;
use crate::content::file::file_id;


/// A `.yentity`'s id and name.
const ID: &str = yog_data::field!(yog_registry::EntityDef, id);
const NAME: &str = yog_data::field!(yog_registry::EntityDef, name);

/// Checks a `.yentity`: its id (the file's name by default), its name into
/// the lang (`entity.<ns>.<path>`), the rest by `yog_registry::EntityDef`.
pub(super) fn compile_entity(cx: &mut Ctx, text: &str, path: &Path, mod_ns: Option<&str>, out: &mut Compiled) -> Option<EntityFile> {
    use yog_data::Value;
    let mut value = match yog_data::text::parse_value(text) {
        Ok(v) => v,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let Value::Map(map) = &mut value else {
        cx.err(1, "an entity is an object: { \"kind\": \"mob\", \"width\": …, \"height\": …, … }");
        return None;
    };
    let given = map.get(ID).and_then(Value::as_str).map(str::to_owned);
    let (ns, p) = file_id(cx, 1, given.as_deref(), path, mod_ns)?;
    let id = yog_ids::join(&ns, &p);
    map.insert(ID, Value::from(id.as_str()));
    // The name → lang, as a block's or an item's.
    if let Some(name) = map.remove(NAME) {
        let key = yog_ids::lang_key(yog_ids::kind::ENTITY, &id);
        match yog_data::from_value::<yog_data::Text>(&name) {
            Ok(read) => match text_key(cx, &mut out.lang, &read.value, &ns, key.clone()) {
                Ok(from) if from != key => out.lang_aliases.push((from, key)),
                Ok(_) => {}
                Err(e) => cx.err(1, format!("{NAME} {e}")),
            },
            Err(problems) => {
                for pr in problems {
                    cx.err(1, format!("{NAME}: {}", pr.message));
                }
            }
        }
    }
    let def = match yog_data::from_value::<yog_registry::EntityDef>(&value) {
        Ok(read) => read.value,
        Err(problems) => {
            for pr in problems {
                cx.err(1, format!("{}: {}", pr.path, pr.message));
            }
            return None;
        }
    };
    let qualify = |s: &Option<String>| s.as_ref().map(|s| yog_ids::qualify_in(&ns, s));
    let mut def = def;
    if let Some(colors) = def.spawn_egg {
        super::looks::spawn_egg(cx.root, out, &ns, &p, colors);
    }
    def.model = qualify(&def.model);
    def.animations = qualify(&def.animations);
    Some(EntityFile {
        id,
        file: cx.file.clone(),
        model: def.model.clone(),
        animations: def.animations.clone(),
        data: yog_data::binary::encode(&yog_data::Schema::write(&def)),
    })
}

/// Checks a `.yfont` and stores it as binary Yog Data.
pub(super) fn compile_font(shown: &str, text: &str, root: &Path, path: &Path, ns: &str, errors: &mut Vec<String>) -> Option<FontFile> {
    let font = match yog_data::from_text::<yog_font::Font>(text) {
        Ok(read) => read.value,
        Err(problems) => {
            for problem in problems.0 {
                errors.push(format!("{shown}: {problem}"));
            }
            return None;
        }
    };
    if let Err(problems) = font.validate() {
        errors.extend(problems.into_iter().map(|problem| format!("{shown}: {problem}")));
        return None;
    }
    let rel = path.strip_prefix(root.join(CONTENT_DIR)).unwrap_or(path).with_extension("");
    let id = format!("{ns}:{}", rel.to_string_lossy().replace('\\', "/"));
    Some(FontFile { id, data: font.to_bytes() })
}

/// Checks a `.ymodel` (its schema, and that it makes a model: parents
/// before children, names once) or a `.yanim` (schema, times).
/// Where a body file is read: the mod's root, its namespace and its textures.
pub(super) struct BodyCx<'a> {
    pub root: &'a Path,
    pub ns: &'a str,
    pub own_textures: &'a std::collections::BTreeSet<String>,
}

pub(super) fn compile_body(cx: &BodyCx, shown: &str, text: &str, kind: Format, path: &Path, errors: &mut Vec<String>) -> Option<BodyFile> {
    let (root, ns) = (cx.root, cx.ns);
    let rel = path.strip_prefix(root.join(CONTENT_DIR)).unwrap_or(path).with_extension("");
    let id = format!("{ns}:{}", rel.to_string_lossy().replace('\\', "/"));
    let report = |errors: &mut Vec<String>, e: yog_data::Errors| {
        for l in e.0 {
            errors.push(format!("{shown}:{l}"));
        }
    };
    let value = if kind == Format::Model {
        let file = match yog_data::from_text::<yog_body::format::ModelFile>(text) {
            Ok(r) => r.value,
            Err(e) => {
                report(errors, e);
                return None;
            }
        };
        if let Err(e) = yog_body::Model::new(&file) {
            errors.push(format!("{shown}: {e}"));
            return None;
        }
        // The mod's own texture must be one of its `.ytexture`s.
        let (tns, tpath) = yog_ids::split(&file.texture);
        if tns == ns && !cx.own_textures.contains(&file.texture) {
            errors.push(format!("{shown}: texture `{}`: no {}/{}", file.texture, crate::yres::RESOURCES_DIR, Format::Texture.file(tpath)));
            return None;
        }
        yog_data::Schema::write(&file)
    } else {
        let file = match yog_data::from_text::<yog_body::format::AnimFile>(text) {
            Ok(r) => r.value,
            Err(e) => {
                report(errors, e);
                return None;
            }
        };
        if let Err(e) = yog_body::anim::Clips::new(&file) {
            errors.push(format!("{shown}: {e}"));
            return None;
        }
        yog_data::Schema::write(&file)
    };
    Some(BodyFile { model: kind == Format::Model, id, data: yog_data::binary::encode(&value) })
}
