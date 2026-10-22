//! One element of a view-model screen: its attributes read (states,
//! bindings, commands), its interactive parts and actions, its bindings'
//! update code, its children.

mod consts;
use consts as st;

use std::collections::HashMap;
use std::fmt::Write as _;

use yog_ids::yui as y;
use yog_ids::yui_attr as at;
use yog_ids::yui_vm as v;

use super::bind::Spot;
use super::binding::{Binding, Bound, Mode, WriteBack, parse_binding};
use super::expr::{Ex, Head, Scope};
use super::state::{Ctx, SCREEN_OWNER, VmGen, path_lit};
use crate::yui::{self, Element, RAW_CALL, err};
use yog_data::Diagnostic;

/// The states an attribute may be prefixed with, and their flags.
const STATES: &[(&str, &str)] = &[
    (st::HOVER, "HOVER"),
    (st::PRESSED, "PRESSED"),
    (st::FOCUSED, "FOCUSED"),
    (st::SELECTED, "SELECTED"),
    (st::CHECKED, "CHECKED"),
    (st::DISABLED, "DISABLED"),
];
/// What separates a state from the attribute it changes: `hover:bg`.
const STATE_SEP: char = ':';
/// The bindings that feed a list rather than set a widget.
const LIST_BINDINGS: [&str; 4] = [v::ITEMS, v::CONTENT, v::KEY, at::SELECTED];
/// The view-model screen's own attributes, not a widget's.
const SCREEN_ATTRS: [&str; 3] = [at::X_VIEW_MODEL, at::X_CREATE, at::X_DATA_TYPE];
/// What `<Image>` and `<Surface>` show until a bound source is set — valid
/// for the static checks.
const IMAGE_STAND_IN: &str = "item:air";
const SURFACE_STAND_IN: &str = "yog:none";

/// What an element's attributes come to.
struct Read {
    /// The static element: its plain attributes and builder calls.
    out: Element,
    raw: usize,
    /// State style patches: flag → fields.
    patches: Vec<(&'static str, Vec<String>)>,
    bindings: Vec<(String, Binding)>,
    command: Option<Binding>,
    list_attrs: HashMap<String, String>,
    /// Whether the element is a list.
    is_list: bool,
}

impl Read {
    /// A builder call on the static element.
    fn push_raw(&mut self, call: String) {
        self.out
            .attrs
            .push((format!("{RAW_CALL}{}", self.raw), call));
        self.raw += 1;
    }
}

/// The built-in element `el` stands for, and whether it's a list.
fn kind_of(el: &Element) -> Result<(&str, bool), Diagnostic> {
    let name = el.name.as_str();
    let kind = match name {
        y::TOGGLE_BUTTON => y::BUTTON,
        y::LIST_VIEW => y::SCROLL_VIEW,
        y::ITEMS_CONTROL
            if el
                .attr(at::ORIENTATION)
                .is_some_and(|o| o.trim() == v::HORIZONTAL) =>
        {
            y::ROW
        }
        y::ITEMS_CONTROL | y::CONTENT_PRESENTER => y::COLUMN,
        v::SCREEN_RESOURCES | v::DATA_TEMPLATE => return err(el, "isn't allowed here"),
        k => k,
    };
    Ok((
        kind,
        matches!(name, y::LIST_VIEW | y::ITEMS_CONTROL | y::CONTENT_PRESENTER),
    ))
}

impl VmGen<'_> {
    /// Compiles `el` at `path` in `ctx`; returns the static element.
    pub(super) fn element(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        path: &[u16],
        items: Option<&str>,
    ) -> Result<Element, Diagnostic> {
        let el = &self.resolve(el)?;
        let (kind, is_list) = kind_of(el)?;
        let mut r = self.read_attrs(el, kind, is_list)?;
        for (flag, fields) in std::mem::take(&mut r.patches) {
            r.push_raw(format!("state_style(::yog_api::ui::StateFlags::{flag}, ::yog_api::ui::StylePatch {{ {}, ..::std::default::Default::default() }})", fields.join(", ")));
        }
        if let Some(id) = r.out.attr(at::ID) {
            self.ids.push(id);
        }
        let spot = self.interactive(ctx, el, kind, path)?;
        self.actions(ctx, el, &spot, &mut r, items)?;
        if is_list {
            if ctx.scope != Scope::Screen {
                return err(el, "lists inside item templates aren't supported");
            }
            self.list(ctx, el, &r.bindings, &r.list_attrs, &spot)?;
        }
        self.bindings(ctx, el, &spot, &mut r)?;
        if !is_list {
            self.children(ctx, el, path, items, &mut r.out)?;
        }
        Ok(r.out)
    }

    /// Reads `el`'s attributes: state paints, bindings and the command,
    /// list attributes, the rest as they are.
    fn read_attrs(&self, el: &Element, kind: &str, is_list: bool) -> Result<Read, Diagnostic> {
        let out = Element {
            name: kind.to_string(),
            attrs: Vec::new(),
            children: Vec::new(),
            line: el.line,
        };
        let mut r = Read {
            out,
            raw: 0,
            patches: Vec::new(),
            bindings: Vec::new(),
            command: None,
            list_attrs: HashMap::new(),
            is_list,
        };
        for (k, val) in &el.attrs {
            let k = k.as_str();
            if k.starts_with(yog_ids::yui_markup::DESIGN) || SCREEN_ATTRS.contains(&k) {
                continue;
            }
            if matches!(k, at::ON_CLICK | at::ON_CHANGE | at::ON_SUBMIT) {
                return err(
                    el,
                    format!(
                        "{k}: view-model screens use command=\"{{x:Bind method()}}\" (and Mode=TwoWay text) instead"
                    ),
                );
            }
            if let Some((state, attr)) = k.split_once(STATE_SEP) {
                let (flag, field) = self.state_paint(el, k, state, attr, val)?;
                match r.patches.iter_mut().find(|(f, _)| *f == flag) {
                    Some((_, fs)) => fs.push(field),
                    None => r.patches.push((flag, vec![field])),
                }
                continue;
            }
            match parse_binding(val) {
                Some(b) => read_binding(
                    el,
                    &mut r,
                    k,
                    b.map_err(|e| Diagnostic::from(format!("{e}")).at_line(el.line))?,
                )?,
                None => plain_attr(el, &mut r, k, val)?,
            }
        }
        Ok(r)
    }

    /// A state-prefixed paint (`hover:bg`): the state's flag, and the style
    /// patch's field.
    fn state_paint(
        &self,
        el: &Element,
        k: &str,
        state: &str,
        attr: &str,
        val: &str,
    ) -> Result<(&'static str, String), Diagnostic> {
        let Some(&(_, flag)) = STATES.iter().find(|(s, _)| *s == state) else {
            return err(
                el,
                format!(
                    "unknown attribute prefix `{state}:` (states: hover, pressed, focused, selected, checked, disabled)"
                ),
            );
        };
        let field = match attr {
            at::BG if yui::is_image_value(val) => {
                let src = yui::image_source(el, k, val.trim(), self.mod_id)?;
                format!(
                    "bg_image: ::std::option::Option::Some(::std::string::String::from({src:?}))"
                )
            }
            at::BG | at::COLOR | at::FOCUS_COLOR => format!(
                "{attr}: ::std::option::Option::Some(::yog_api::ui::Color(0x{:08X}))",
                yui::color_attr(el, k, val)?
            ),
            at::SHADOW => format!(
                "{attr}: ::std::option::Option::Some({})",
                yui::boolean(el, k, val)?
            ),
            _ => {
                return err(
                    el,
                    format!(
                        "{k}: only paint attributes (bg, color, shadow, focus_color) take a state prefix"
                    ),
                );
            }
        };
        Ok((flag, field))
    }

    /// The view's text box and scroller `el` is (screen scope only), and
    /// where it is.
    fn interactive<'s>(
        &mut self,
        ctx: &Ctx,
        el: &Element,
        kind: &'s str,
        path: &'s [u16],
    ) -> Result<Spot<'s>, Diagnostic> {
        let screen = ctx.scope == Scope::Screen;
        let text_box = (kind == y::TEXT_BOX && screen).then(|| {
            let _ = writeln!(self.parts, "        parts.text_box({});", path_lit(path));
            self.n_text_boxes += 1;
            self.n_text_boxes - 1
        });
        if kind == y::SCROLL_VIEW && !screen {
            return err(el, "scroll views inside item templates aren't supported");
        }
        let scroller = (kind == y::SCROLL_VIEW).then(|| {
            let _ = writeln!(self.parts, "        parts.scroller({});", path_lit(path));
            self.n_scrollers += 1;
            self.n_scrollers - 1
        });
        Ok(Spot {
            kind,
            path,
            text_box,
            scroller,
        })
    }

    /// A text box's two-way text and submit command; any other element's
    /// command and two-way toggle, as an action.
    fn actions(
        &mut self,
        ctx: &Ctx,
        el: &Element,
        spot: &Spot,
        r: &mut Read,
        items: Option<&str>,
    ) -> Result<(), Diagnostic> {
        if spot.kind == y::TEXT_BOX {
            return self.text_box_events(el, spot, r);
        }
        let toggle = r
            .bindings
            .iter()
            .find(|(k, b)| k == at::CHECKED && b.mode == Mode::TwoWay)
            .map(|(_, b)| b.clone());
        if r.command.is_none() && toggle.is_none() {
            return Ok(());
        }
        let owner = ctx.owner;
        let slot = self.next_cmd(owner);
        r.push_raw(format!(
            "action(::yog_api::ui::Action {{ owner: {owner}, slot: {slot}, index: 0 }})"
        ));
        let mut body = String::new();
        if let Some(t) = &toggle {
            if ctx.scope == Scope::Template {
                return err(
                    el,
                    "Mode=TwoWay toggles in item templates aren't supported — use a command",
                );
            }
            body.push_str(&toggled(el, t)?);
        }
        if let Some(c) = &r.command {
            body.push_str(&self.command(el, c, ctx.scope, items)?);
        }
        let index = if owner == SCREEN_OWNER { ".." } else { "index" };
        let pat = format!(
            "::yog_kit::ViewEvent::Action(::yog_api::ui::Action {{ owner: {owner}, slot: {slot}, {index} }})"
        );
        self.dispatch.push((pat, body.trim_end().to_string()));
        if ctx.scope == Scope::Screen {
            let _ = writeln!(
                self.parts,
                "        parts.focusable({});",
                path_lit(spot.path)
            );
        }
        Ok(())
    }

    /// A text box's events: its text written back, its command on submit.
    fn text_box_events(&mut self, el: &Element, spot: &Spot, r: &Read) -> Result<(), Diagnostic> {
        let tb = spot.text_box.ok_or_else(|| {
            Diagnostic::from("text boxes in item templates aren't supported".to_string())
                .at_line(el.line)
        })?;
        if let Some((_, b)) = r.bindings.iter().find(|(k, _)| k == at::TEXT) {
            match b.write_back("text", "text")? {
                WriteBack::Stmt(write) => self.dispatch.push((
                    format!("::yog_kit::ViewEvent::TextChanged {{ slot: {tb}, text }}"),
                    write,
                )),
                WriteBack::Nowhere => {
                    return err(
                        el,
                        "Mode=TwoWay needs a field (text=\"{x:Bind search, Mode=TwoWay}\") or BindBack=method",
                    );
                }
                WriteBack::OneWay => {}
            }
        }
        if let Some(c) = &r.command {
            let stmt = self.command(el, c, Scope::Screen, None)?;
            self.dispatch.push((
                format!("::yog_kit::ViewEvent::TextSubmitted {{ slot: {tb}, text }}"),
                format!("let _ = text; {stmt}"),
            ));
        }
        Ok(())
    }

    /// The element's bindings: a component's properties rebuilding it,
    /// a bound source's stand-in, the rest applied one by one.
    fn bindings(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        spot: &Spot,
        r: &mut Read,
    ) -> Result<(), Diagnostic> {
        // A component's bound properties: when any changes, the instance is
        // rebuilt with the new values (then its other bindings re-applied).
        let rebuild = if yui::is_custom_component(spot.kind)
            && r.bindings.iter().any(|(k, _)| !yui::is_common_attr(k))
        {
            Some(self.component_props(ctx, el, &r.out, &mut r.bindings, spot.path)?)
        } else {
            None
        };
        let slots_before = self.n_slots;
        if r.bindings.iter().any(|(k, _)| k == at::SOURCE) {
            let stand_in = match spot.kind {
                y::IMAGE => Some(IMAGE_STAND_IN),
                y::SURFACE => Some(SURFACE_STAND_IN),
                _ => None,
            };
            r.out
                .attrs
                .extend(stand_in.map(|s| (at::SOURCE.to_string(), s.to_string())));
        }
        for (k, b) in r
            .bindings
            .iter()
            .filter(|(k, _)| !LIST_BINDINGS.contains(&k.as_str()))
        {
            self.bind_attr(ctx, el, spot, k, b)?;
        }
        if let Some(marker) = rebuild {
            let resets: String = (slots_before..self.n_slots)
                .map(|i| format!(" {}b{i}.reset();", ctx.fields))
                .collect();
            ctx.update = ctx.update.replace(&marker, &resets);
        }
        Ok(())
    }

    /// The element's children, each at its path; the screen's resources left out.
    fn children(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        path: &[u16],
        items: Option<&str>,
        out: &mut Element,
    ) -> Result<(), Diagnostic> {
        let mut i = 0u16;
        for child in &el.children {
            if child.name.contains(yog_ids::sep::DOT) {
                if child.name == v::SCREEN_RESOURCES && path.is_empty() {
                    continue;
                }
                return err(child, "isn't allowed here");
            }
            let p: Vec<u16> = path.iter().copied().chain([i]).collect();
            out.children.push(self.element(ctx, child, &p, items)?);
            i += 1;
        }
        Ok(())
    }
}

/// A two-way toggle's statement: its field flipped, or its `BindBack`
/// method called with the flipped value.
fn toggled(el: &Element, t: &Binding) -> Result<String, Diagnostic> {
    match (&t.bind_back, t.expr()) {
        (Some(m), Some(e)) => Ok(format!(
            "let __v = !{}; vm.{m}(__v); ",
            e.emit(Scope::Screen, false)?
        )),
        (None, Some(e)) if e.is_place() => {
            let p = e.emit(Scope::Screen, false)?;
            Ok(format!("{p} = !{p}; "))
        }
        _ => err(
            el,
            "checked Mode=TwoWay needs a bool field or BindBack=method",
        ),
    }
}

/// A bound attribute: the command, a list's binding (on a list only), or
/// one to apply.
fn read_binding(el: &Element, r: &mut Read, k: &str, b: Binding) -> Result<(), Diagnostic> {
    if k == v::COMMAND {
        r.command = Some(b);
        return Ok(());
    }
    if LIST_BINDINGS.contains(&k) && !r.is_list {
        return err(
            el,
            format!("{k} is for ListView, ItemsControl and ContentPresenter"),
        );
    }
    r.bindings.push((k.to_string(), b));
    Ok(())
}

/// A plain attribute: a list's own, a fixed visibility or check, a
/// `.yres` text (bound, so it follows the language), or one as it is.
fn plain_attr(el: &Element, r: &mut Read, k: &str, val: &str) -> Result<(), Diagnostic> {
    let res_text =
        val.starts_with(yog_ids::sep::TYPED) && !val.starts_with(ESCAPED_AT) && val.len() > 1;
    match k {
        v::COMMAND => return err(el, "command needs {x:Bind method(...)}"),
        _ if LIST_BINDINGS.contains(&k) => {
            return err(el, format!("{k} needs a binding: {k}=\"{{x:Bind ...}}\""));
        }
        at::ITEM_HEIGHT | at::ORIENTATION if r.is_list => {
            r.list_attrs.insert(k.into(), val.to_string());
        }
        at::VISIBLE => {
            let b = yui::boolean(el, k, val)?;
            r.push_raw(format!("visible({b})"));
        }
        at::CHECKED if r.out.name == y::BUTTON => {
            let fixed = Ex::Post(Head::Lit(yui::boolean(el, k, val)?.to_string()), Vec::new());
            r.bindings
                .push((k.into(), Binding::one_way(val, Bound::Expr(fixed))));
        }
        at::TEXT | at::PLACEHOLDER | at::TOOLTIP if res_text => {
            // Looked up on every update, so it follows the language.
            r.bindings.push((
                k.into(),
                Binding::one_way(val, Bound::Rust(yui::text_attr(el, k, val)?)),
            ));
        }
        _ => r.out.attrs.push((k.to_string(), val.to_string())),
    }
    Ok(())
}

/// A literal `@` at a text's start: `@@`.
const ESCAPED_AT: &str = yog_ids::concatcp!(yog_ids::sep::TYPED, yog_ids::sep::TYPED);
