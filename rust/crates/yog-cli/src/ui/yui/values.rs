//! The values of `.yui` attributes: numbers, flags, colors, texts, image
//! sources, names — read and turned into Rust.

use std::collections::HashSet;
use std::fmt::Write as _;

use yog_gfx::image::ImageSource;

use super::{Element, err};
use yog_data::Diagnostic;

/// A number attribute.
pub(crate) fn number(el: &Element, key: &str, value: &str) -> Result<f32, Diagnostic> {
    value
        .trim()
        .parse::<f32>()
        .or_else(|_| err(el, format_args!("{key}=\"{value}\": expected a number")))
}

/// A yes/no attribute: `true` or `false`.
pub(crate) fn boolean(el: &Element, key: &str, value: &str) -> Result<bool, Diagnostic> {
    match value.trim() {
        yog_ids::yui_value::TRUE => Ok(true),
        yog_ids::yui_value::FALSE => Ok(false),
        _ => err(
            el,
            format_args!("{key}=\"{value}\": expected true or false"),
        ),
    }
}

/// `#RRGGBB`, `#AARRGGBB` or `0xAARRGGBB` → `0xAARRGGBB` (`Color::parse`;
/// a file's color is written with its `#` or `0x`, not as a decimal).
pub fn color(value: &str) -> Option<u32> {
    yog_ui::attr::written_color(value).map(|c| c.0)
}

/// What a color written in hex starts with: `#RRGGBB`.
pub const HEX_MARK: &str = yog_ui::attr::HEX_MARKS[0];

/// A color attribute: a literal (see [`color`]) or `@<Set>.<name>`, a color
/// of `resources/<Set>.yres` (see [`set_resource_refs`]).
pub(crate) fn color_attr(el: &Element, key: &str, value: &str) -> Result<u32, Diagnostic> {
    if let Some(reference) = value.trim().strip_prefix('@') {
        return resource_color(reference).ok_or_else(|| {
            Diagnostic::from(format!(
                "<{}> {key}=\"{value}\": no color `{reference}` in resources/*.yres",
                el.name
            ))
            .at_line(el.line)
        });
    }
    color(value).ok_or_else(|| {
        Diagnostic::from(format!(
            "<{}> {key}=\"{value}\": expected #RRGGBB, #AARRGGBB or @<Set>.<name>",
            el.name
        ))
        .at_line(el.line)
    })
}

/// Whether `@<Set>.<name>` names a text of the mod's `.yres` resources.
pub(super) fn is_resource_text(reference: &str) -> bool {
    RESOURCE_REFS.with(|r| r.borrow().texts.contains_key(reference.trim()))
}

/// The color `@<Set>.<name>` names in the mod's `.yres` resources.
pub(super) fn resource_color(reference: &str) -> Option<u32> {
    RESOURCE_REFS.with(|r| r.borrow().colors.get(reference.trim()).copied())
}

/// `value`, one of `options`.
pub(super) fn pick<'v>(
    el: &Element,
    key: &str,
    value: &str,
    options: &[&'v str],
) -> Result<&'v str, Diagnostic> {
    options
        .iter()
        .find(|o| **o == value.trim())
        .copied()
        .ok_or_else(|| {
            Diagnostic::from(format!(
                "<{}> {key}=\"{value}\": expected one of {}",
                el.name,
                options.join(", ")
            ))
            .at_line(el.line)
        })
}

/// The Rust expression for a text attribute, its `@` forms as everywhere
/// in Yog Data (`yog_data::typed`): `"@Strings.title"` is a text of
/// `resources/Strings.yres` (see [`set_resource_refs`]), `"@lang:gui.done"`
/// any translation key, `"@item:minecraft:stick"` (`@block:`, `@entity:`)
/// that content's name — each looked up when the tree is built
/// (`::yog_api::tr`, so it follows the game's language); `"@@..."` is a
/// literal text starting with `@`; anything else is the literal text.
pub fn text_expr(value: &str) -> Result<String, Diagnostic> {
    if value.trim() == "@" || !value.starts_with('@') {
        return Ok(format!("{value:?}"));
    }
    let tr = |key: &str| format!("::yog_api::tr({key:?})");
    match yog_data::typed::classify(value.trim()) {
        Ok(yog_data::Value::Str(s)) => Ok(format!("{s:?}")),
        Ok(yog_data::Value::Lang(key)) => Ok(tr(&key)),
        Ok(yog_data::Value::Res(r)) => {
            RESOURCE_REFS.with(|refs| Ok(refs.borrow().text(&r).map(|(k, _)| tr(k))?))
        }
        Ok(yog_data::Value::Ref(r))
            if matches!(
                r.kind.as_str(),
                yog_ids::kind::ITEM | yog_ids::kind::BLOCK | yog_ids::kind::ENTITY
            ) =>
        {
            // A block's state (`[facing=north]`) isn't part of its name.
            let id = r.id.split(STATE_OPEN).next().unwrap_or(&r.id);
            Ok(tr(&yog_ids::lang_key(&r.kind, id)))
        }
        Ok(other) => Err(format!("{} isn't a text", other.kind_name()).into()),
        Err(_) if !value.contains(':') => Err(format!(
            "write a translation key as `@lang:{}`, a .yres text as `@Set.name`, a literal `@` as `@@`",
            value[1..].trim()
        ).into()),
        Err(e) => Err(e.into()),
    }
}

/// Where a block id's state begins: `minecraft:furnace[facing=north]`.
const STATE_OPEN: char = '[';

/// [`text_expr`] for attribute `key` of `el`, a problem saying where.
pub(crate) fn text_attr(el: &Element, key: &str, value: &str) -> Result<String, Diagnostic> {
    text_expr(value).map_err(|e| {
        Diagnostic::from(format!("<{}> {key}=\"{value}\": {e}", el.name)).at_line(el.line)
    })
}

thread_local! {
    /// The mod's `.yres` resources, for `@<Set>.<name>` in attributes.
    static RESOURCE_REFS: std::cell::RefCell<crate::yres::Refs> = Default::default();
}

/// Makes the mod's compiled `.yres` resources visible to the generator:
/// `text="@Strings.title"` and `source="@Icons.logo"` resolve through them.
pub fn set_resource_refs(refs: &crate::yres::Refs) {
    RESOURCE_REFS.with(|r| *r.borrow_mut() = refs.clone());
}

/// A Rust constant name for an event/widget id: `mymod:btn-play` → `MYMOD_BTN_PLAY`.
pub fn const_name(value: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push('_');
            }
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
            out.push(c.to_ascii_uppercase());
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

/// Names in first-seen order, each once.
#[derive(Debug, Default)]
pub(crate) struct Names {
    list: Vec<String>,
    seen: HashSet<String>,
}

impl Names {
    pub(crate) fn push(&mut self, name: &str) {
        if !self.seen.contains(name) {
            self.seen.insert(name.to_string());
            self.list.push(name.to_string());
        }
    }

    pub(crate) fn as_slice(&self) -> &[String] {
        &self.list
    }

    pub(crate) fn into_vec(self) -> Vec<String> {
        self.list
    }
}

/// `pub const NAME: &str = "value";` lines for `values`, the names from
/// [`const_name`] (a `_` appended to repeats).
pub(crate) fn str_consts(values: &[String]) -> String {
    let mut out = String::new();
    let mut used: HashSet<String> = HashSet::new();
    for v in values {
        let mut name = const_name(v);
        while used.contains(&name) {
            name.push('_');
        }
        let _ = writeln!(out, "    pub const {name}: &str = {v:?};");
        used.insert(name);
    }
    out
}

/// Whether a `bg` value is an image (`sprite:panel`, `mymod:gui/frame`,
/// an `@Set.name` image) rather than a color (`#RRGGBB`, `@Colors.name`).
pub(crate) fn is_image_value(v: &str) -> bool {
    let v = v.trim();
    match v.strip_prefix('@') {
        Some(name) => RESOURCE_REFS.with(|r| r.borrow().images.contains_key(name)),
        None => !v.starts_with('#') && v.contains(':'),
    }
}

/// An image source attribute (`<Image source>`, `bg="sprite:..."`): checks
/// it and resolves `@Set.name` and `mod:` → the source the host draws.
pub(crate) fn image_source(
    el: &Element,
    attr: &str,
    source: &str,
    mod_id: Option<&str>,
) -> Result<String, Diagnostic> {
    let line = el.line;
    let kind = el.name.as_str();
    let mod_id = mod_id.map(str::to_owned);
    let resolved;
    let source = match source.strip_prefix('@') {
        Some(name) => {
            resolved = RESOURCE_REFS
                .with(|r| r.borrow().images.get(name).cloned())
                .ok_or_else(|| {
                    Diagnostic::from(format!(
                        "<{kind}> {attr}=\"{source}\": no image `{name}` in resources/*.yres"
                    ))
                    .at_line(line)
                })?;
            resolved.as_str()
        }
        None => source,
    };
    let parsed = ImageSource::parse(source).ok_or_else(|| {
        Diagnostic::from(format!(
            "<{kind}> {attr}=\"{source}\": expected item:<id>, block:<id>, \
             sprite:<name>, mod:<path> or <namespace>:<path>"
        ))
        .at_line(line)
    })?;
    let source = match parsed {
        ImageSource::ModPath(path) => match &mod_id {
            Some(id) => yog_gfx::image::texture_id(id, &path),
            None => {
                return Err(Diagnostic::from(format!(
                    "<{kind}> {attr}=\"{source}\": mod: needs the mod id (no yog.toml found)"
                ))
                .at_line(line));
            }
        },
        ImageSource::Sprite(name)
            if !name.contains(':') && yog_gfx::image::sprite(&name).is_none() =>
        {
            let names: Vec<&str> = yog_gfx::image::SPRITES
                .iter()
                .map(|s| s.name.as_str())
                .collect();
            return Err(Diagnostic::from(format!(
                "<{kind}> {attr}=\"{source}\": unknown sprite (known: {})",
                names.join(", ")
            ))
            .at_line(line));
        }
        ImageSource::Texture(_) => {
            if let Some(alias) = yog_gfx::image::suggest_alias(source) {
                return Err(Diagnostic::from(format!(
                    "<{kind}> {attr}=\"{source}\" is a raw resource path, which \
                     changes between Minecraft versions — write {attr}=\"{alias}\""
                ))
                .at_line(line));
            }
            source.to_string()
        }
        _ => source.to_string(),
    };
    Ok(source)
}

/// A `font` attribute: a Yog font, or a game font by Yog's alias — never
/// the game's own id.
pub(super) fn font_attr<'a>(el: &Element, v: &'a str) -> Result<&'a str, Diagnostic> {
    let font = v.trim();
    match yog_ids::split(font) {
        (yog_ids::ns::MINECRAFT, name) if !font.is_empty() => err(
            el,
            format!(
                "font=\"{font}\": the game's fonts are named by Yog's alias, {}",
                yog_ids::font_alias(name)
            ),
        ),
        _ => Ok(font),
    }
}
