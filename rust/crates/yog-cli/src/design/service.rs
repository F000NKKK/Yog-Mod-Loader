//! The designers as `yog serve` methods: what a tool (Yog-IDLE) asks to draw,
//! list or change, by the name it calls it and with its parameters.

use std::path::Path;
use yog_ids::serve_method as M;
use yog_ids::serve_param as P;

use base64::Engine as _;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{
    advancements, book, codebehind, content, custom, font_copy, images, project, recipe_kinds,
    refs, tags, worldgen, yui_codegen,
};

type Reply = Result<Value, Value>;

/// A parameter, read as its type.
fn param<T: DeserializeOwned>(params: &Value, name: &str) -> Result<T, Value> {
    serde_json::from_value(params.get(name).cloned().unwrap_or(Value::Null))
        .map_err(|e| json!(format!("`{name}`: {e}")))
}

fn ok<T: Serialize>(v: T) -> Reply {
    serde_json::to_value(v).map_err(|e| json!(e.to_string()))
}

fn done<T: Serialize>(r: Result<T, String>) -> Reply {
    r.map_err(Value::from).and_then(ok)
}

/// A picture's `width` and `height` parameters.
fn size(params: &Value) -> Result<(u32, u32), Value> {
    Ok((param(params, P::WIDTH)?, param(params, P::HEIGHT)?))
}

/// The texture the `.ytexture` at `file` is: `@texture:<mod>:<path>`.
fn texture_file_id(file: &Path) -> Option<String> {
    let root = project::mod_root(file)?;
    let rel = file
        .strip_prefix(root.join(yog_ids::project::RESOURCES))
        .ok()?
        .to_string_lossy()
        .replace('\\', yog_ids::sep::PATH);
    let id = yog_formats::texture::id_of(&project::mod_id(&root)?, &rel)?;
    Some(format!("{}{id}", yog_ids::reference::TEXTURE))
}

/// What design method `method` does with `params`; `None`: no such method.
pub fn dispatch(method: &str, params: &Value) -> Option<Reply> {
    let path = || param::<String>(params, P::PATH);
    let file = |f: fn(&Path) -> Reply| path().and_then(|p| f(Path::new(&p)));
    let with = |name: &str, f: &dyn Fn(&Path, &str) -> Reply| -> Reply {
        let (p, v) = (path()?, param::<String>(params, name)?);
        f(Path::new(&p), &v)
    };
    Some(match method {
        M::RECIPE_KINDS => file(|p| ok(recipe_kinds::all(super::mod_root(p).as_deref()))),
        M::RECIPE_KIND_CREATE => with(P::NAME, &|p, name| done(recipe_kinds::create(p, name))),
        M::WORLDGEN_PREVIEW => param::<String>(params, P::TEXT)
            .and_then(|t| done(worldgen::render(&t, param(params, P::SIZE)?))),
        M::YUI_CODEGEN => path().and_then(|p| done(yui_codegen::codegen(&p))),
        M::CONTENT_MOD_ID => file(|p| ok(super::mod_root(p).and_then(|r| project::mod_id(&r)))),
        M::CONTENT_TEXTURE | M::BODY_TEXTURE => {
            with(P::TEXTURE, &|p, t| done(content::texture(p, t)))
        }
        M::CONTENT_SAVE_TEXTURE => with(P::TEXTURE, &|p, t| {
            let png = base64::engine::general_purpose::STANDARD
                .decode(param::<String>(params, P::PNG)?)
                .map_err(|e| json!(e.to_string()))?;
            done(content::write_texture(p, t, png))
        }),
        M::CONTENT_SAVE_TEXTURE_ANIMATION => with(P::TEXTURE, &|p, t| {
            done(content::write_animation(
                p,
                t,
                param::<Option<content::Animation>>(params, "animation")?.as_ref(),
            ))
        }),
        M::CONTENT_TEXTURES => file(|p| ok(content::textures(p))),
        M::CONTENT_STRINGS => file(|p| ok(content::strings(p))),
        M::CONTENT_SOUND_GROUPS => file(|p| ok(content::sound_groups(p))),
        M::CONTENT_TOOL_MATERIALS => file(|p| ok(content::tool_materials(p))),
        M::CONTENT_ARMOR_MATERIALS => file(|p| ok(content::armor_materials(p))),
        M::CONTENT_REFS => with(P::KIND, &|p, k| ok(refs::refs(p, k))),
        M::CONTENT_DEFS => with(P::KIND, &|p, k| ok(refs::own_defs(p, k))),
        M::CONTENT_BLOCK_KINDS => Ok(refs::block_kinds()),
        M::CONTENT_TAGS => with(P::REGISTRY, &|p, r| ok(content::tags(p, r))),
        M::CONTENT_PARENT_MODEL => with(P::ID, &|p, id| done(content::parent_model(p, id))),
        M::CONTENT_ITEM_POTION_COLOR => file(|p| ok(content::item_potion_color(p))),
        // The mod's copy of a game font (by Yog's alias), made if it has none: `{ id, file }`.
        M::FONT_COPY => with(P::ALIAS, &|p, alias| done(font_copy::copy(p, alias))),
        M::CONTENT_IMPORT => with(P::KIND, &|p, kind| {
            let model = param::<Option<String>>(params, P::MODEL)?;
            done(content::import(
                p,
                kind,
                &param::<String>(params, P::SOURCE)?,
                model.as_deref(),
                param(params, P::FORCE)?,
            ))
        }),
        // The texture a `.ytexture` file is (`@texture:<mod>:<path>`), or null outside a mod's resources.
        M::TEXTURE_FILE_ID => file(|p| ok(texture_file_id(p))),
        // A `.ytexture`'s `animation` (as the file has it) of a `width × height` picture, every frame resolved.
        M::TEXTURE_ANIMATION_RESOLVE => size(params).and_then(|size| {
            let a = yog_data::read_value::<yog_formats::texture::Animation>(
                &yog_data::json::from_json(&params[P::ANIMATION]),
            )
            .map_err(|e| json!(format!("{e:?}")))?;
            ok(content::Animation::of(&a, size))
        }),
        // A resolved animation (as `texture_animation_resolve` gives it) as the file keeps it.
        M::TEXTURE_ANIMATION_FORMAT => size(params).and_then(|size| {
            let a: content::Animation = param(params, P::ANIMATION)?;
            ok(yog_data::json::to_json(&yog_data::Schema::write(
                &a.to_format(size),
            )))
        }),
        M::ITEM_TAG_ITEMS => with(P::TAG, &|p, t| ok(tags::item_tag(p, t))),
        M::YUI_IMAGE_SOURCES => file(|p| ok(images::candidates(p))),
        M::YUI_IMAGE_PREVIEW => with(P::SOURCE, &|p, s| {
            ok(images::preview(p, s, param(params, P::SIZE)?))
        }),
        M::YUI_DESIGNER_RENDER => param(params, P::ARGS).and_then(|a| done(super::render(&a))),
        M::YUI_DESIGNER_EVENT => param(params, P::ARGS).and_then(|a| ok(super::event(&a))),
        M::YUI_DESIGNER_RESET => path().map(|p| {
            super::reset(&p);
            Value::Null
        }),
        M::YUI_DESIGNER_COMPONENTS => file(|p| {
            let root = if project::path(p).is_file() {
                p.to_path_buf()
            } else {
                super::mod_root(p).ok_or("not inside a mod (no src/ folder)")?
            };
            ok(custom::status(&root))
        }),
        M::YUI_EVENT_HANDLER => with(P::EVENT, &|p, e| done(codebehind::ensure_handler(p, e))),
        M::BOOK_PAGE_PREVIEW => param(params, P::ARGS).and_then(|a| ok(book::page_preview(&a))),
        M::BOOK_DESIGNER_RENDER => param(params, P::ARGS).and_then(|a| ok(book::render(&a))),
        M::BOOK_DESIGNER_CLOSE => path().map(|p| {
            book::close(&p);
            Value::Null
        }),
        M::ADVANCEMENT_RENDER => param(params, P::ARGS).and_then(|a| ok(advancements::render(&a))),
        M::YDATA_TEXT => file(|p| {
            let bytes = std::fs::read(p).map_err(|e| json!(e.to_string()))?;
            let value = yog_data::binary::decode(&bytes).map_err(|e| json!(e.to_string()))?;
            ok(yog_data::text::to_string_pretty(&value))
        }),
        _ => return None,
    })
}
