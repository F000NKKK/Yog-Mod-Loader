//! The designers as `yog serve` methods: what a tool (Yog-IDLE) asks to draw,
//! list or change, by the name it calls it and with its parameters.

use yog_ids::serve_param as P;
use std::path::Path;

use base64::Engine as _;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{advancements, book, codebehind, content, custom, font_copy, images, project, recipe_kinds, refs, tags, worldgen, yui_codegen};

type Reply = Result<Value, Value>;

/// A parameter, read as its type.
fn param<T: DeserializeOwned>(params: &Value, name: &str) -> Result<T, Value> {
    serde_json::from_value(params.get(name).cloned().unwrap_or(Value::Null)).map_err(|e| json!(format!("`{name}`: {e}")))
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
    let rel = file.strip_prefix(root.join(yog_ids::project::RESOURCES)).ok()?.to_string_lossy().replace('\\', yog_ids::sep::PATH);
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
        "recipe_kinds" => file(|p| ok(recipe_kinds::all(super::mod_root(p).as_deref()))),
        "recipe_kind_create" => with(P::NAME, &|p, name| done(recipe_kinds::create(p, name))),
        "worldgen_preview" => param::<String>(params, P::TEXT).and_then(|t| done(worldgen::render(&t, param(params, P::SIZE)?))),
        "yui_codegen" => path().and_then(|p| done(yui_codegen::codegen(&p))),
        "content_mod_id" => file(|p| ok(super::mod_root(p).and_then(|r| project::mod_id(&r)))),
        "content_texture" | "body_texture" => with(P::TEXTURE, &|p, t| done(content::texture(p, t))),
        "content_save_texture" => with(P::TEXTURE, &|p, t| {
            let png = base64::engine::general_purpose::STANDARD.decode(param::<String>(params, P::PNG)?).map_err(|e| json!(e.to_string()))?;
            done(content::write_texture(p, t, png))
        }),
        "content_save_texture_animation" => with(P::TEXTURE, &|p, t| done(content::write_animation(p, t, param::<Option<content::Animation>>(params, "animation")?.as_ref()))),
        "content_textures" => file(|p| ok(content::textures(p))),
        "content_strings" => file(|p| ok(content::strings(p))),
        "content_sound_groups" => file(|p| ok(content::sound_groups(p))),
        "content_tool_materials" => file(|p| ok(content::tool_materials(p))),
        "content_armor_materials" => file(|p| ok(content::armor_materials(p))),
        "content_refs" => with(P::KIND, &|p, k| ok(refs::refs(p, k))),
        "content_defs" => with(P::KIND, &|p, k| ok(refs::own_defs(p, k))),
        "content_block_kinds" => Ok(refs::block_kinds()),
        "content_tags" => with(P::REGISTRY, &|p, r| ok(content::tags(p, r))),
        "content_parent_model" => with(P::ID, &|p, id| done(content::parent_model(p, id))),
        "content_item_potion_color" => file(|p| ok(content::item_potion_color(p))),
        // The mod's copy of a game font (by Yog's alias), made if it has none: `{ id, file }`.
        "font_copy" => with(P::ALIAS, &|p, alias| done(font_copy::copy(p, alias))),
        "content_import" => with(P::KIND, &|p, kind| {
            let model = param::<Option<String>>(params, P::MODEL)?;
            done(content::import(p, kind, &param::<String>(params, P::SOURCE)?, model.as_deref(), param(params, P::FORCE)?))
        }),
        // The texture a `.ytexture` file is (`@texture:<mod>:<path>`), or null outside a mod's resources.
        "texture_file_id" => file(|p| ok(texture_file_id(p))),
        // A `.ytexture`'s `animation` (as the file has it) of a `width × height` picture, every frame resolved.
        "texture_animation_resolve" => size(params).and_then(|size| {
            let a = yog_data::read_value::<yog_formats::texture::Animation>(&yog_data::json::from_json(&params[P::ANIMATION])).map_err(|e| json!(format!("{e:?}")))?;
            ok(content::Animation::of(&a, size))
        }),
        // A resolved animation (as `texture_animation_resolve` gives it) as the file keeps it.
        "texture_animation_format" => size(params).and_then(|size| {
            let a: content::Animation = param(params, P::ANIMATION)?;
            ok(yog_data::json::to_json(&yog_data::Schema::write(&a.to_format(size))))
        }),
        "item_tag_items" => with(P::TAG, &|p, t| ok(tags::item_tag(p, t))),
        "yui_image_sources" => file(|p| ok(images::candidates(p))),
        "yui_image_preview" => with(P::SOURCE, &|p, s| ok(images::preview(p, s, param(params, P::SIZE)?))),
        "yui_designer_render" => param(params, P::ARGS).and_then(|a| done(super::render(&a))),
        "yui_designer_event" => param(params, P::ARGS).and_then(|a| ok(super::event(&a))),
        "yui_designer_reset" => path().map(|p| {
            super::reset(&p);
            Value::Null
        }),
        "yui_designer_components" => file(|p| {
            let root = if project::path(p).is_file() { p.to_path_buf() } else { super::mod_root(p).ok_or("not inside a mod (no src/ folder)")? };
            ok(custom::status(&root))
        }),
        "yui_event_handler" => with(P::EVENT, &|p, e| done(codebehind::ensure_handler(p, e))),
        "book_page_preview" => param(params, P::ARGS).and_then(|a| ok(book::page_preview(&a))),
        "book_designer_render" => param(params, P::ARGS).and_then(|a| ok(book::render(&a))),
        "book_designer_close" => path().map(|p| {
            book::close(&p);
            Value::Null
        }),
        "advancement_render" => param(params, P::ARGS).and_then(|a| ok(advancements::render(&a))),
        "ydata_text" => file(|p| {
            let bytes = std::fs::read(p).map_err(|e| json!(e.to_string()))?;
            let value = yog_data::binary::decode(&bytes).map_err(|e| json!(e.to_string()))?;
            ok(yog_data::text::to_string_pretty(&value))
        }),
        _ => return None,
    })
}
