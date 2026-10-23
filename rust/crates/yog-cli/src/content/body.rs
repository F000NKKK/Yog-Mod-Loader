//! `.yentity`, `.ymodel`/`.yanim` and `.yfont` files: checked and built.

use crate::content::Compiled;
use crate::content::block_item::text_key;
use crate::content::ctx::problem;
use crate::content::ctx::{Ctx, Shared};
use crate::content::file::file_id;
use crate::content::files::{BodyFile, EntityFile, FontFile};
use crate::diagnostics::Diagnostic;
use std::path::Path;
use yog_ids::Format;

/// A `.yentity`'s id and name.
const ID: &str = yog_data::field!(yog_registry::EntityDef, id);
const NAME: &str = yog_data::field!(yog_registry::EntityDef, name);

/// Checks a `.yentity`: its id (the file's name by default), its name into
/// the lang (`entity.<ns>.<path>`), the rest by `yog_registry::EntityDef`.
pub(super) fn compile_entity(
    cx: &mut Ctx,
    text: &str,
    path: &Path,
    mod_ns: Option<&str>,
    out: &mut Compiled,
) -> Option<EntityFile> {
    use yog_data::Value;
    let mut value = match yog_data::text::parse_value(text) {
        Ok(v) => v,
        Err(e) => {
            cx.err(e.at.line as usize, e.message);
            return None;
        }
    };
    let Value::Map(map) = &mut value else {
        cx.err(
            1,
            "an entity is an object: { \"kind\": \"mob\", \"width\": …, \"height\": …, … }",
        );
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
pub(super) fn compile_font(
    cx: &Shared,
    shown: &str,
    text: &str,
    path: &Path,
    errors: &mut Vec<Diagnostic>,
) -> Option<FontFile> {
    let mut font = match yog_data::from_text::<yog_font::Font>(text) {
        Ok(read) => read.value,
        Err(problems) => {
            for found in &problems.0 {
                errors.push(found.clone().in_file(shown));
            }
            return None;
        }
    };
    font.normalize();
    if let Err(problems) = font.validate() {
        errors.extend(
            problems
                .into_iter()
                .map(|found| problem(found).in_file(shown)),
        );
        return None;
    }
    Some(FontFile {
        id: cx.content_id(path),
        data: font.to_bytes(),
    })
}

/// What's wrong with `.ymodel` `file`: it doesn't make a model (parents
/// before children, names once), or names a texture or a video of the mod
/// that it hasn't.
fn model_problem(cx: &Shared, file: &yog_body::format::ModelFile) -> Result<(), String> {
    yog_body::Model::new(file).map_err(|e| e.to_string())?;
    let (tns, tpath) = yog_ids::split(&file.texture);
    if tns == cx.ns && !cx.own_textures.contains(&file.texture) {
        return Err(format!(
            "texture `{}`: no {}/{}",
            file.texture,
            crate::yres::RESOURCES_DIR,
            Format::Texture.file(tpath)
        ));
    }
    if let Some((vns, vpath)) = file.video.as_deref().map(yog_ids::split)
        && vns == cx.ns
    {
        let rel = vpath.replace(yog_ids::sep::DOT, yog_ids::sep::PATH);
        let video = cx
            .root
            .join(crate::yres::RESOURCES_DIR)
            .join(yog_formats::video::DIR)
            .join(Format::Video.file(&rel));
        if !video.is_file() {
            return Err(format!(
                "video `{vns}:{vpath}`: no {}",
                crate::shown_path(cx.root, &video)
            ));
        }
    }
    Ok(())
}

/// Checks a `.ymodel` (its schema, and that it makes a model: parents
/// before children, names once) or a `.yanim` (schema, times).
pub(super) fn compile_body(
    cx: &Shared,
    shown: &str,
    text: &str,
    kind: Format,
    path: &Path,
    errors: &mut Vec<Diagnostic>,
) -> Option<BodyFile> {
    let id = cx.content_id(path);
    let report = |errors: &mut Vec<Diagnostic>, e: yog_data::Errors| {
        for l in &e.0 {
            errors.push(l.clone().in_file(shown));
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
        if let Err(e) = model_problem(cx, &file) {
            errors.push(problem(e).in_file(shown));
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
            errors.push(problem(e.to_string()).in_file(shown));
            return None;
        }
        yog_data::Schema::write(&file)
    };
    Some(BodyFile {
        model: kind == Format::Model,
        id,
        data: yog_data::binary::encode(&value),
    })
}
