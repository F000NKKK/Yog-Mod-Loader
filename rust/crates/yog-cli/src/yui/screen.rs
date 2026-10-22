//! A plain `<Screen>`'s code: its frame, its theme, its widget tree, and
//! the functions that lay it out, draw it and open it.

use yog_ids::yui_attr as at;
use yog_ui::layout::Align;

use super::anchor::Anchor;
use super::values::{boolean, number, pick, str_consts};
use super::widget::Gen;
use super::{Element, UI_USE, parse_xml};
use crate::rust_code::{f32_lit, fill};
use yog_data::Diagnostic;

/// A `<Screen>`'s frame: its size, where it sits, how it opens.
pub(crate) struct Frame {
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// Its `anchor`'s word (`center` when it gives none).
    pub(crate) anchor: &'static str,
    pub(crate) modal: bool,
    pub(crate) pause: bool,
    pub(crate) live: bool,
}

/// A `<Screen>`'s frame: `width` × `height` (the vanilla container's size
/// by default), `anchor` (centred), `modal` (yes), `pause` and `live` (no).
pub(crate) fn screen_frame(root: &Element) -> Result<Frame, Diagnostic> {
    let size = |key: &str, default: f32| {
        root.attr(key)
            .map(|v| number(root, key, v))
            .transpose()
            .map(|v| v.unwrap_or(default))
    };
    let flag = |key: &str, default: bool| {
        root.attr(key)
            .map(|v| boolean(root, key, v))
            .transpose()
            .map(|v| v.unwrap_or(default))
    };
    let words = Anchor::words();
    let anchor = pick(
        root,
        at::ANCHOR,
        root.attr(at::ANCHOR)
            .unwrap_or(yog_ids::screen_anchor::CENTER),
        &words,
    )?;
    Ok(Frame {
        width: size(at::WIDTH, yog_ui::widget::SCREEN_W)?,
        height: size(at::HEIGHT, yog_ui::widget::SCREEN_H)?,
        anchor,
        modal: flag(at::MODAL, true)?,
        pause: flag(at::PAUSE, false)?,
        live: flag(at::LIVE, false)?,
    })
}

/// The theme `<Screen theme="...">` names, if any.
pub(crate) fn screen_theme(root: &Element) -> Result<Option<yog_ui::theme::Theme>, Diagnostic> {
    let Some(id) = root
        .attr(yog_ids::yui_attr::THEME)
        .map(str::trim)
        .filter(|t| !t.is_empty())
    else {
        return Ok(None);
    };
    yog_ui::theme::Theme::builtin(id).map(Some).ok_or_else(|| {
        Diagnostic::from(format!(
            "<Screen> theme=\"{id}\": no such theme (built in: {})",
            yog_ui::theme::Theme::BUILTIN.join(", ")
        ))
        .at_line(root.line)
    })
}

/// Gives every element of `el`'s subtree its theme defaults (what it
/// doesn't set itself) — for screens without styles to merge first.
pub(crate) fn apply_theme(el: &mut Element, theme: &yog_ui::theme::Theme) {
    theme.apply(&el.name, &mut el.attrs);
    for c in &mut el.children {
        apply_theme(c, theme);
    }
}

/// Generates the `.g.rs` source for a `.yui` document. `name` is the screen's
/// path under `src/` without the extension (`mod_list`, `screens/options`):
/// what the code-behind passes to `ui_include!`.
#[cfg(test)]
pub(crate) fn generate(src: &str, name: &str) -> Result<String, Diagnostic> {
    generate_for(src, name, None).map(|(code, _)| code)
}

/// [`generate`] for the mod `mod_id` (resolves `mod:` image sources), also
/// returning `line N: ...` warnings.
pub fn generate_for(
    src: &str,
    name: &str,
    mod_id: Option<&str>,
) -> Result<(String, Vec<Diagnostic>), Diagnostic> {
    generate_with(src, name, mod_id, &[])
}

/// [`generate_for`] with the mod's recipe layouts, for `<RecipeArea>`s:
/// a component's code, a view-model screen's, or a plain screen's.
pub(crate) fn generate_with(
    src: &str,
    name: &str,
    mod_id: Option<&str>,
    layouts: &[yog_formats::book::layout::RecipeLayout],
) -> Result<(String, Vec<Diagnostic>), Diagnostic> {
    let mut root = parse_xml(src)?;
    // A book's kind of page or a recipe layout isn't code: the build reads it as data.
    if crate::book_page::is_book_ui(&root) {
        return Ok((String::new(), Vec::new()));
    }
    crate::recipe_area::expand(&mut root, layouts, mod_id.unwrap_or(yog_ids::ns::MINECRAFT))?;
    crate::yui_grid::normalize(&mut root)?;
    if root.name == yog_ids::yui::COMPONENT {
        return crate::yui_component::generate(&root, name, mod_id);
    }
    if root.attr(at::X_VIEW_MODEL).is_some() {
        return crate::yui_vm::generate(&root, name, mod_id);
    }
    if let Some(theme) = screen_theme(&root)? {
        apply_theme(&mut root, &theme);
    }
    // Plain screens have nothing to warn about (yet); view-model screens do.
    Ok((screen_code(&root, name, mod_id)?, Vec::new()))
}

/// Where the screen's content goes on a `screen_w` × `screen_h` GUI, as the
/// game places its screens: one bigger than the window keeps its size and
/// is cut at the window's edges (evenly, centred), and a container's
/// picture stays over its real slots.
fn place(anchor: Anchor) -> String {
    // `trunc`: the game's whole-pixel `(width - backgroundWidth) / 2`.
    let at = |align: Align, screen: &str, size: &str| match align {
        Align::Start => f32_lit(0.0),
        Align::Center => format!("(({screen} - {size}) / 2.0).trunc()"),
        Align::End => format!("{screen} - {size}"),
    };
    match anchor {
        Anchor::Fill => {
            let whole = f32_lit(WHOLE);
            format!("content.w(Units::Percent({whole})).h(Units::Percent({whole})).at(0.0, 0.0)")
        }
        Anchor::At(h, v) => format!(
            "content.at({}, {})",
            at(h, "screen_w", "WIDTH"),
            at(v, "screen_h", "HEIGHT")
        ),
    }
}

/// All of the window, in percent.
const WHOLE: f32 = 100.0;

/// The generated code of a plain screen.
const TEMPLATE: &str = include_str!("screen.g.rs.in");

/// A plain `<Screen>`'s code.
fn screen_code(root: &Element, name: &str, mod_id: Option<&str>) -> Result<String, Diagnostic> {
    if root.name != yog_ids::yui::SCREEN {
        return Err(Diagnostic::from(format!(
            "the root element must be <Screen>, not <{}>",
            root.name
        ))
        .at_line(root.line));
    }
    let id = root
        .attr(at::ID)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            Diagnostic::from("<Screen> needs an id (the UI id, e.g. \"mymod:main\")".to_string())
                .at_line(root.line)
        })?
        .to_string();
    let frame = screen_frame(root)?;
    let mut codegen = Gen::new(mod_id);
    // The screen's own id is the UI id, not a widget id.
    let mut content_root = root.clone();
    content_root.attrs.retain(|(k, _)| k != at::ID);
    let body = codegen.widget(&content_root, false, 0)?;
    let anchor = Anchor::parse(frame.anchor).unwrap_or(Anchor::At(Align::Center, Align::Center));
    let values = [
        ("NAME", name.to_string()),
        ("ID_LIT", format!("{id:?}")),
        ("WIDTH", f32_lit(frame.width)),
        ("HEIGHT", f32_lit(frame.height)),
        ("EVENTS", str_consts(codegen.events.as_slice())),
        ("IDS", str_consts(codegen.ids.as_slice())),
        ("UI_USE", UI_USE.to_string()),
        ("BODY", body),
        ("ANCHOR", frame.anchor.to_string()),
        ("PLACE", place(anchor)),
        ("MODAL", frame.modal.to_string()),
        ("PAUSE", frame.pause.to_string()),
    ];
    Ok(fill(TEMPLATE, &values))
}
