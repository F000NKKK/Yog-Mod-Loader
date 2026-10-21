//! A `.yui` element's builder expression: the widget it makes, positioned
//! and set up by its attributes, its children after it.

use std::fmt::Write as _;

use yog_gfx::image::Stretch;
use yog_ids::yui as y;
use yog_ids::yui_attr as at;
use yog_ui::attr::Attr;

use super::attr_code;
use super::elements::{
    COMMON_ATTRS, CONTAINERS, LEAVES, component_expr, is_component_name, kind_attrs, wpf_hint,
};
use super::values::{
    Names, color_attr, font_attr, image_source, is_image_value, number, text_attr,
};
use super::{Element, RAW_CALL, err};
use crate::rust_code::f32_lit;

/// What generating a tree finds: the events its widgets raise, their ids.
pub(crate) struct Gen {
    pub(crate) events: Names,
    pub(crate) ids: Names,
    /// The mod's id (from `yog.toml`), for `mod:` image sources.
    mod_id: Option<String>,
}

/// What an `on_*` attribute of a component's starts with: an event it raises.
const EVENT_PREFIX: &str = "on_";
/// What a `<Surface>`'s source of the mod's own starts with: `mod:grid`.
const MOD_SCHEME: &str = yog_ids::concatcp!(yog_ids::image_scheme::MOD, yog_ids::sep::NAMESPACE);
/// How many `<InvSlot>` shows when it doesn't say.
const ONE: u32 = 1;

/// The builder expression of `el` and its subtree, and the ids found —
/// the static half of a view-model screen.
pub(crate) fn static_widget(
    el: &Element,
    in_canvas: bool,
    depth: usize,
    mod_id: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let mut codegen = Gen::new(mod_id);
    let expr = codegen.widget(el, in_canvas, depth)?;
    Ok((expr, codegen.ids.into_vec()))
}

impl Gen {
    pub(crate) fn new(mod_id: Option<&str>) -> Self {
        Gen {
            events: Names::default(),
            ids: Names::default(),
            mod_id: mod_id.map(str::to_string),
        }
    }

    /// The builder expression for `el`. `in_canvas`: the parent positions
    /// every child absolutely (`Screen`/`Canvas`) — a missing x/y means 0.
    pub(crate) fn widget(
        &mut self,
        el: &Element,
        in_canvas: bool,
        depth: usize,
    ) -> Result<String, String> {
        let kind = el.name.as_str();
        let is_component = check_element(el, depth)?;
        let mut expr = self.base(el, is_component)?;
        let indent = "    ".repeat(depth + 2);
        let mut calls: Vec<String> = Vec::new();
        position(el, in_canvas, &mut calls)?;
        for (key, value) in &el.attrs {
            self.attr_call(el, is_component, key, value, &mut calls)?;
        }
        for c in calls {
            let _ = write!(expr, "\n{indent}.{c}");
        }
        let canvas = matches!(kind, y::SCREEN | y::CANVAS);
        for child in &el.children {
            let c = self.widget(child, canvas, depth + 1)?;
            let _ = write!(expr, "\n{indent}.child({c})");
        }
        Ok(expr)
    }

    /// The element's own widget, before its attributes.
    fn base(&mut self, el: &Element, is_component: bool) -> Result<String, String> {
        let text = || text_attr(el, at::TEXT, el.attr(at::TEXT).unwrap_or_default());
        Ok(match el.name.as_str() {
            y::SCREEN | y::CANVAS | y::COLUMN => "widget::panel(FlexDir::Column)".to_string(),
            y::ROW => "widget::panel(FlexDir::Row)".to_string(),
            y::SCROLL_VIEW => "widget::scroll_view()".to_string(),
            y::GRID => "widget::grid()".to_string(),
            y::LABEL => format!("widget::label({})", text()?),
            y::BUTTON => format!("widget::button({})", text()?),
            y::TEXT_BOX => format!("widget::text_box({})", text()?),
            y::ITEM_SLOT => format!(
                "widget::item_slot({:?})",
                el.attr(at::ITEM).unwrap_or_default()
            ),
            y::MACHINE => machine(el),
            y::INV_SLOT => inv_slot(el)?,
            y::SLOT => slot(el)?,
            y::PLAYER_INVENTORY => "widget::player_inventory()".to_string(),
            y::IMAGE => self.image(el)?,
            y::SURFACE => self.surface(el)?,
            _ if is_component => component_expr(el)?,
            _ => "widget::spacer()".to_string(),
        })
    }

    /// The builder call attribute `key` makes, if any: what the view-model
    /// generator added, ids and events, texts, fonts, backgrounds and
    /// colors, the common attributes (`yog_ui::attr`).
    fn attr_call(
        &mut self,
        el: &Element,
        is_component: bool,
        key: &str,
        v: &str,
        calls: &mut Vec<String>,
    ) -> Result<(), String> {
        if key.starts_with(RAW_CALL) {
            // Builder calls added by the view-model generator.
            calls.push(v.to_string());
            return Ok(());
        }
        if is_component && !COMMON_ATTRS.contains(&key) {
            // A component's own property (passed to `build` above); an
            // `on_*` one names an event like `on_click` does.
            if key.starts_with(EVENT_PREFIX) && !v.trim().is_empty() {
                self.events.push(v);
            }
            return Ok(());
        }
        let call = match key {
            at::ID => {
                self.ids.push(v);
                format!("{key}({v:?})")
            }
            at::ON_CLICK | at::ON_CHANGE | at::ON_SUBMIT => {
                self.events.push(v);
                format!("{key}({v:?})")
            }
            at::PLACEHOLDER | at::TOOLTIP => format!("{key}({})", text_attr(el, key, v)?),
            at::FONT => format!("{key}({:?})", font_attr(el, v)?),
            at::BG if is_image_value(v) => format!(
                "bg_image({:?})",
                image_source(el, key, v.trim(), self.mod_id.as_deref())?
            ),
            at::BG | at::COLOR | at::FOCUS_COLOR => format!(
                "{key}(::yog_api::ui::Color(0x{:08X}))",
                color_attr(el, key, v)?
            ),
            _ => match Attr::parse(key, v) {
                Ok(Some(attr)) => match attr_code::call(&attr) {
                    Some(c) => c,
                    None => return Ok(()),
                },
                // Kind-specific, handled with the widget (or Screen metadata).
                Ok(None) => return Ok(()),
                Err(want) => return err(el, format_args!("{key}=\"{v}\": expected {want}")),
            },
        };
        calls.push(call);
        Ok(())
    }

    /// `widget::surface(...)` for a `<Surface>`: the id its drawing is
    /// registered under (`ns:path`; `mod:path` = the mod's namespace).
    fn surface(&mut self, el: &Element) -> Result<String, String> {
        let line = el.line;
        let source = el
            .attr(at::SOURCE)
            .ok_or_else(|| format!("line {line}: <Surface> needs a source (the id its code registers, e.g. source=\"mod:grid\")"))?
            .trim();
        let id = match (source.strip_prefix(MOD_SCHEME), &self.mod_id) {
            (Some(path), Some(m)) => yog_ids::join(m, path),
            (Some(_), None) => {
                return Err(format!(
                    "line {line}: <Surface> source=\"{source}\": mod: needs the mod id (no yog.toml found)"
                ));
            }
            (None, _) => source.to_string(),
        };
        if !id.contains(yog_ids::sep::NAMESPACE) || !crate::content::valid_id(&id) {
            return Err(format!(
                "line {line}: <Surface> source=\"{source}\": expected <namespace>:<name> or mod:<name> \
                 (lower case a-z, 0-9, `_`, `-`, `.`, `/`)"
            ));
        }
        Ok(format!("widget::surface({id:?})"))
    }

    /// `widget::image(...)` for an `<Image>`: its `source` checked and
    /// `mod:` resolved, plus `stretch`/`uv`/`tint`.
    fn image(&mut self, el: &Element) -> Result<String, String> {
        let line = el.line;
        let source = el
            .attr(at::SOURCE)
            .ok_or_else(|| {
                format!("line {line}: <Image> needs a source (e.g. source=\"item:apple\")")
            })?
            .trim();
        let source = image_source(el, at::SOURCE, source, self.mod_id.as_deref())?;
        let mut expr = format!("widget::image({source:?})");
        if let Some(v) = el.attr(at::STRETCH) {
            let names = || Stretch::ALL.map(|(_, n)| n).join(", ");
            let s = Stretch::parse(v).map(Stretch::name).ok_or_else(|| {
                format!(
                    "line {line}: <Image> stretch=\"{v}\": expected one of {}",
                    names()
                )
            })?;
            let _ = write!(expr, ".stretch(::yog_api::ui::Stretch::{s})");
        }
        if let Some(v) = el.attr(at::UV) {
            let nums: Vec<f32> = v
                .split([',', ' '])
                .filter(|p| !p.trim().is_empty())
                .map(|p| number(el, at::UV, p))
                .collect::<Result<_, _>>()?;
            let [u, vv, w, h] = nums[..] else {
                return Err(format!(
                    "line {line}: <Image> uv=\"{v}\": expected \"u v width height\""
                ));
            };
            let _ = write!(
                expr,
                ".region([{}, {}, {}, {}])",
                f32_lit(u),
                f32_lit(vv),
                f32_lit(w),
                f32_lit(h)
            );
        }
        if let Some(v) = el.attr(at::TINT) {
            let _ = write!(
                expr,
                ".tint(::yog_api::ui::Color(0x{:08X}))",
                color_attr(el, at::TINT, v)?
            );
        }
        Ok(expr)
    }
}

/// Whether `el` may be where it is: a known element (or a component's
/// PascalCase name), a `<Screen>` only at the root, children only in a
/// container, only attributes it takes. Whether it's a component.
fn check_element(el: &Element, depth: usize) -> Result<bool, String> {
    let kind = el.name.as_str();
    let is_container = CONTAINERS.contains(&kind);
    // Any other PascalCase element is one of the mod's own components
    // (`yog_api::ui_component!`), looked up by name when the tree is built.
    let is_component = !is_container && !LEAVES.contains(&kind) && is_component_name(kind);
    if !is_container && !LEAVES.contains(&kind) && !is_component {
        let (_, containers) = CONTAINERS.split_at(1);
        return Err(format!(
            "line {}: unknown element <{kind}> (expected one of {}, {}, or a custom component's PascalCase name)",
            el.line,
            containers.join(", "),
            LEAVES.join(", ")
        ));
    }
    if kind == y::SCREEN && depth > 0 {
        return Err(format!(
            "line {}: <Screen> must be the root element",
            el.line
        ));
    }
    if !is_container && !el.children.is_empty() {
        return Err(format!("line {}: <{kind}> can't have children", el.line));
    }
    let foreign = |key: &str| {
        !key.starts_with(RAW_CALL)
            && !COMMON_ATTRS.contains(&key)
            && !kind_attrs(kind).contains(&key)
    };
    if !is_component && let Some((key, _)) = el.attrs.iter().find(|(k, _)| foreign(k)) {
        let hint = wpf_hint(key)
            .map(|h| format!(" — .yui writes it `{h}`"))
            .unwrap_or_default();
        return Err(format!(
            "line {}: <{kind}> has no attribute `{key}`{hint}",
            el.line
        ));
    }
    Ok(is_component)
}

/// A screen's size, and where the element is: always in a canvas, else
/// when it says.
fn position(el: &Element, in_canvas: bool, calls: &mut Vec<String>) -> Result<(), String> {
    if el.name == y::SCREEN {
        for (key, call) in [(at::WIDTH, at::W), (at::HEIGHT, at::H)] {
            if let Some(v) = el.attr(key) {
                calls.push(format!(
                    "{call}(Units::Px({}))",
                    f32_lit(number(el, key, v)?)
                ));
            }
        }
    }
    let x = el.attr(at::X).map(|v| number(el, at::X, v)).transpose()?;
    let y = el.attr(at::Y).map(|v| number(el, at::Y, v)).transpose()?;
    if in_canvas || x.is_some() || y.is_some() {
        calls.push(format!(
            "at({}, {})",
            f32_lit(x.unwrap_or(0.0)),
            f32_lit(y.unwrap_or(0.0))
        ));
    }
    Ok(())
}

/// What a `<RecipeArea>` leaves (`crate::recipe_area`): which slots make what.
fn machine(el: &Element) -> String {
    let places: Vec<String> = el
        .attr(at::PLACES)
        .unwrap_or_default()
        .split(',')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (slot, rest) = p.split_once('/').unwrap_or((p, "0=0"));
            let (i, inv) = rest.split_once('=').unwrap_or(("0", "0"));
            format!("({slot:?}, {i}, {inv})")
        })
        .collect();
    format!(
        "widget::machine({:?}, &[{}], {})",
        el.attr(at::KIND).unwrap_or_default(),
        places.join(", "),
        el.attr(at::RESULT).unwrap_or("0")
    )
}

/// An `<InvSlot>`: its item and how many.
fn inv_slot(el: &Element) -> Result<String, String> {
    let count = match el.attr(at::COUNT) {
        Some(c) => c.trim().parse::<u32>().map_err(|_| {
            format!(
                "line {}: <InvSlot> count=\"{c}\": expected a whole number",
                el.line
            )
        })?,
        None => ONE,
    };
    Ok(format!(
        "widget::inv_slot({:?}, {count})",
        el.attr(at::ITEM).unwrap_or_default()
    ))
}

/// A `<Slot>`: the container's slot of its number, and what it takes.
fn slot(el: &Element) -> Result<String, String> {
    let index = el.attr(at::INDEX).unwrap_or_default();
    let index = index.trim().parse::<usize>().map_err(|_| {
        format!(
            "line {}: <Slot> index=\"{index}\": expected the container slot's number (0, 1, ...)",
            el.line
        )
    })?;
    let accepts: Vec<&str> = el
        .attr(at::ACCEPTS)
        .unwrap_or_default()
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|a| !a.is_empty())
        .collect();
    for a in &accepts {
        let id = a.strip_prefix(yog_ids::sep::TAG).unwrap_or(a);
        let ok = id
            .split_once(yog_ids::sep::NAMESPACE)
            .is_some_and(|(ns, path)| yog_ids::valid_namespace(ns) && yog_ids::valid_path(path));
        if !ok {
            return Err(format!(
                "line {}: <Slot> accepts=\"{a}\": expected an item id (minecraft:redstone) or an item tag (#c:dusts), with its namespace",
                el.line
            ));
        }
    }
    Ok(if accepts.is_empty() {
        format!("widget::slot({index})")
    } else {
        format!("widget::slot_accepting({index}, &{accepts:?})")
    })
}
