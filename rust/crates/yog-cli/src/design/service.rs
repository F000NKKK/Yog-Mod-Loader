//! The designers as `yog serve` methods: what a tool (Yog-IDLE) asks to draw,
//! list or change, by the name it calls it and with its parameters.

use std::path::Path;
use yog_ids::serve_method as M;
use yog_ids::serve_param as P;

use base64::Engine as _;
use yog_data::{Schema, Value};

use crate::serve::ServiceError;

use super::{
    drawing::images, editors::advancements, editors::book, editors::content, editors::recipe_kinds,
    editors::tags, editors::worldgen, fonts::font_copy, screens::codebehind, screens::custom,
    screens::yui_codegen, sources::project, sources::refs,
};

type Reply = crate::serve::Reply;

/// A parameter, read as its type.
fn param<T: Schema>(params: &Value, name: &str) -> Result<T, ServiceError> {
    yog_data::read_value(params.get(name).unwrap_or(&Value::Null))
        .map_err(|e| ServiceError::param(name, e))
}

/// A parameter that's markup's text (`item:apple`, `@res:Images.logo`): as spelled.
fn spelled(params: &Value, name: &str) -> Result<String, ServiceError> {
    use yog_data::with::{Spelled, With};
    let v = params.get(name).unwrap_or(&Value::Null);
    let arena = yog_data::Bump::new();
    let mut cx = yog_data::Reader::new();
    <Spelled as With<String>>::read(&v.borrow(&arena), &mut cx)
        .filter(|_| cx.problems.is_empty())
        .ok_or_else(|| ServiceError::param(name, cx.problems))
}

fn ok<T: Schema>(v: T) -> Reply {
    Ok(v.write())
}

fn done<T: Schema, E: Into<ServiceError>>(r: Result<T, E>) -> Reply {
    r.map_err(Into::into).and_then(ok)
}

/// A picture's `width` and `height` parameters.
fn size(params: &Value) -> Result<(u32, u32), ServiceError> {
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

/// The `sound` parameter as `sound_data` takes it: a resource (`@res:Sounds.x`,
/// as the IDE sends one) written back as text, else the text itself.
fn sound_param(params: &Value) -> Result<String, ServiceError> {
    match params.get(P::SOUND) {
        Some(Value::Ref(r))
            if let Some(yog_data::Meaning::Res(r)) = yog_data::Meaning::of(r.clone()) =>
        {
            Ok(r.to_string())
        }
        Some(Value::Str(s)) => Ok(s.clone()),
        _ => Err(ServiceError::from(format!(
            "`{}`: a sound, `@res:Sounds.x` or a game's file",
            P::SOUND
        ))),
    }
}

/// A method's parameters, and the ways the methods read them.
struct Call<'a> {
    params: &'a Value,
}

impl Call<'_> {
    /// The `path` parameter: the file the method is about.
    fn path(&self) -> Result<String, ServiceError> {
        param::<String>(self.params, P::PATH)
    }

    /// `f` of the `path` file.
    fn file(&self, f: fn(&Path) -> Reply) -> Reply {
        self.path().and_then(|p| f(Path::new(&p)))
    }

    /// `f` of the `path` file and the string parameter `name`.
    fn with(&self, name: &str, f: &dyn Fn(&Path, &str) -> Reply) -> Reply {
        let (p, v) = (self.path()?, param::<String>(self.params, name)?);
        f(Path::new(&p), &v)
    }

    /// `f` of the `path` file and the `texture` parameter — a texture's id,
    /// `@texture:<ns>:<path>` or just `<ns>:<path>`.
    fn texture(&self, f: &dyn Fn(&Path, &str) -> Reply) -> Reply {
        let p = self.path()?;
        let t: yog_data::Id<yog_data::kinds::Texture> = param(self.params, P::TEXTURE)?;
        f(Path::new(&p), &yog_ids::qualify(&t.id))
    }
}

/// What design method `method` does with `params`; `None`: no such method.
pub(crate) fn dispatch(method: &str, params: &Value) -> Option<Reply> {
    let c = Call { params };
    content_method(method, &c).or_else(|| designer_method(method, &c))
}

/// The methods of a mod's content: its textures, the pickers' choices, imports.
fn content_method(method: &str, c: &Call) -> Option<Reply> {
    Some(match method {
        M::CONTENT_MOD_ID => c.file(|p| ok(super::mod_root(p).and_then(|r| project::mod_id(&r)))),
        M::CONTENT_TEXTURE | M::BODY_TEXTURE => c.texture(&|p, t| done(content::texture(p, t))),
        M::SOUND_LIST => c.file(|p| done(super::editors::sound::sound_list(p))),
        M::VIDEO_LIST => c.file(|p| done(super::editors::video::video_list(p))),
        M::VIDEO_PREVIEW => c.path().and_then(|p| {
            let video = param::<yog_formats::video::Video>(c.params, P::VALUE)?;
            done(super::editors::video::preview(Path::new(&p), &video))
        }),
        M::VIDEO_FRAME => param::<String>(c.params, P::KEY).and_then(|key| {
            done(super::editors::video::frame(
                &key,
                param(c.params, P::FRAME)?,
            ))
        }),
        M::SOUND_DATA => c.path().and_then(|p| {
            done(super::editors::sound::sound_data(
                Path::new(&p),
                &sound_param(c.params)?,
            ))
        }),
        M::CONTENT_SAVE_TEXTURE => c.texture(&|p, t| {
            let png = base64::engine::general_purpose::STANDARD
                .decode(param::<String>(c.params, P::PNG)?)
                .map_err(|e| ServiceError::from(e.to_string()))?;
            done(content::write_texture(p, t, png))
        }),
        M::CONTENT_SAVE_TEXTURE_ANIMATION => c.texture(&|p, t| {
            done(content::write_animation(
                p,
                t,
                param::<Option<content::Animation>>(c.params, P::ANIMATION)?.as_ref(),
            ))
        }),
        M::CONTENT_TEXTURES => c.file(|p| ok(content::textures(p))),
        M::CONTENT_STRINGS => c.file(|p| ok(content::strings(p))),
        M::CONTENT_SOUND_GROUPS => c.file(|p| ok(content::sound_groups(p))),
        M::CONTENT_TOOL_MATERIALS => c.file(|p| ok(content::tool_materials(p))),
        M::CONTENT_ARMOR_MATERIALS => c.file(|p| ok(content::armor_materials(p))),
        M::CONTENT_REFS => c.with(P::KIND, &|p, k| ok(refs::refs(p, k))),
        M::CONTENT_DEFS => c.with(P::KIND, &|p, k| ok(refs::own_defs(p, k))),
        M::CONTENT_BLOCK_KINDS => Ok(refs::block_kinds()),
        M::CONTENT_TAGS => c.with(P::REGISTRY, &|p, r| ok(content::tags(p, r))),
        M::CONTENT_PARENT_MODEL => c.with(P::ID, &|p, id| done(content::parent_model(p, id))),
        M::CONTENT_ITEM_POTION_COLOR => c.file(|p| ok(content::item_potion_color(p))),
        // The mod's copy of a game font (by Yog's alias), made if it has none: `{ id, file }`.
        M::FONT_COPY => c.with(P::ALIAS, &|p, alias| done(font_copy::copy(p, alias))),
        M::CONTENT_IMPORT => c.with(P::KIND, &|p, kind| {
            let model = param::<Option<String>>(c.params, P::MODEL)?;
            done(content::import(
                p,
                kind,
                &param::<String>(c.params, P::SOURCE)?,
                model.as_deref(),
                param(c.params, P::FORCE)?,
            ))
        }),
        // The texture a `.ytexture` file is (`@texture:<mod>:<path>`), or null outside a mod's resources.
        M::TEXTURE_FILE_ID => c.file(|p| ok(texture_file_id(p))),
        // A `.ytexture`'s `animation` (as the file has it) of a `width × height` picture, every frame resolved.
        M::TEXTURE_ANIMATION_RESOLVE => size(c.params).and_then(|size| {
            let a: yog_formats::texture::Animation = param(c.params, P::ANIMATION)?;
            ok(content::Animation::of(&a, size))
        }),
        // A resolved animation (as `texture_animation_resolve` gives it) as the file keeps it.
        M::TEXTURE_ANIMATION_FORMAT => size(c.params).and_then(|size| {
            let a: content::Animation = param(c.params, P::ANIMATION)?;
            ok(a.to_format(size))
        }),
        M::ITEM_TAG_ITEMS => c.with(P::TAG, &|p, t| ok(tags::item_tag(p, t))),
        _ => return None,
    })
}

/// The designers' methods: drawing screens, books, advancements and worldgen, their events.
fn designer_method(method: &str, c: &Call) -> Option<Reply> {
    Some(match method {
        M::RECIPE_KINDS => c.file(|p| ok(recipe_kinds::all(super::mod_root(p).as_deref()))),
        M::RECIPE_KIND_CREATE => c.with(P::NAME, &|p, name| done(recipe_kinds::create(p, name))),
        M::WORLDGEN_PREVIEW => param::<String>(c.params, P::TEXT)
            .and_then(|t| done(worldgen::render(&t, param(c.params, P::SIZE)?))),
        M::YUI_CODEGEN => c.path().and_then(|p| done(yui_codegen::codegen(&p))),
        M::YUI_IMAGE_SOURCES => c.file(|p| ok(images::candidates(p))),
        M::YUI_IMAGE_PREVIEW => c.path().and_then(|p| {
            let source = spelled(c.params, P::SOURCE)?;
            ok(images::preview(
                Path::new(&p),
                &source,
                param(c.params, P::SIZE)?,
            ))
        }),
        M::YUI_DESIGNER_RENDER => param(c.params, P::ARGS).and_then(|a| done(super::render(&a))),
        M::YUI_DESIGNER_EVENT => param(c.params, P::ARGS).and_then(|a| ok(super::event(&a))),
        M::YUI_DESIGNER_RESET => c.path().map(|p| {
            super::reset(&p);
            Value::Null
        }),
        M::YUI_DESIGNER_COMPONENTS => c.file(|p| {
            let root = if project::path(p).is_file() {
                p.to_path_buf()
            } else {
                super::mod_root(p).ok_or("not inside a mod (no src/ folder)")?
            };
            ok(custom::status(&root))
        }),
        M::YUI_EVENT_HANDLER => c.with(P::EVENT, &|p, e| done(codebehind::ensure_handler(p, e))),
        M::BOOK_PAGE_PREVIEW => param(c.params, P::ARGS).and_then(|a| ok(book::page_preview(&a))),
        M::BOOK_DESIGNER_RENDER => param(c.params, P::ARGS).and_then(|a| ok(book::render(&a))),
        M::BOOK_DESIGNER_CLOSE => c.path().map(|p| {
            book::close(&p);
            Value::Null
        }),
        M::ADVANCEMENT_RENDER => param::<advancements::AdvancementArgs>(c.params, P::ARGS)
            .and_then(|a| ok(advancements::render(&a.resolved()))),

        _ => return None,
    })
}
