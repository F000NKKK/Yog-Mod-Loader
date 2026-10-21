//! Lists (`ListView`, `ItemsControl`, `ContentPresenter`): their item
//! template and empty state, the update that fills them, the selection
//! written back; and a component's bound properties.

use std::collections::HashMap;
use std::fmt::Write as _;

use yog_ids::yui as y;
use yog_ids::yui_attr as at;
use yog_ids::yui_vm as v;

use super::WIDGET;
use super::bind::Spot;
use super::binding::{Binding, Mode, WriteBack};
use super::expr::{INDEX, Scope};
use super::state::{Ctx, INSTANCE_FIELDS, ITEM_TREE, SCREEN_OWNER, VIEW_FIELDS, VmGen, path_lit};
use crate::yui::{self, Element, RAW_VALUE, UI_USE, err};

/// `Some(x)` / `None` as generated code writes them.
fn option(v: Option<String>) -> String {
    v.map_or("::std::option::Option::None".to_string(), |v| {
        format!("::std::option::Option::Some({v})")
    })
}

/// A list's item template and its empty state's element.
struct Templates<'e> {
    item: &'e Element,
    empty: Option<&'e Element>,
}

/// What a list is filled from, and how it knows its items.
struct Source {
    /// `items` or `content`: the attribute.
    attr: &'static str,
    /// The items as Rust (by reference when a field).
    items: String,
    /// Each item's key as Rust (template scope).
    key: String,
    /// The binding, for comments.
    raw: String,
}

/// The single element inside a template/empty property element.
fn only_child(el: &Element) -> Result<&Element, String> {
    let c = match el.children.as_slice() {
        [t] if t.name == v::DATA_TEMPLATE => t,
        _ => el,
    };
    match c.children.as_slice() {
        [one] => Ok(one),
        _ => err(el, "needs exactly one child element"),
    }
}

/// A list's `<X.ItemTemplate>` (or a bare `<DataTemplate>`) and `<X.Empty>`.
fn templates(el: &Element) -> Result<Templates<'_>, String> {
    let name = el.name.as_str();
    let (mut item, mut empty) = (None, None);
    for c in &el.children {
        match c
            .name
            .strip_prefix(name)
            .and_then(|n| n.strip_prefix(yog_ids::sep::MEMBER))
        {
            Some(v::ITEM_TEMPLATE | v::CONTENT_TEMPLATE) => item = Some(only_child(c)?),
            Some(v::EMPTY) => empty = Some(only_child(c)?),
            _ if c.name == v::DATA_TEMPLATE => item = Some(only_child(c)?),
            _ => {
                return err(
                    c,
                    format!(
                        "isn't allowed in <{name}> (expected <{name}.ItemTemplate> or <{name}.Empty>)"
                    ),
                );
            }
        }
    }
    let item = item.ok_or_else(|| {
        format!(
            "line {}: <{name}> needs an item template (<{name}.ItemTemplate><DataTemplate>...)",
            el.line
        )
    })?;
    Ok(Templates { item, empty })
}

/// The binding named `key` among `bindings`.
fn bound<'b>(bindings: &'b [(String, Binding)], key: &str) -> Option<&'b Binding> {
    bindings.iter().find(|(k, _)| k == key).map(|(_, b)| b)
}

/// What list `el` is filled from: its items (or content) and their keys.
fn source(el: &Element, bindings: &[(String, Binding)]) -> Result<Source, String> {
    let attr = if el.name == y::CONTENT_PRESENTER {
        v::CONTENT
    } else {
        v::ITEMS
    };
    let Some(b) = bound(bindings, attr) else {
        return err(el, format!("needs {attr}=\"{{x:Bind ...}}\""));
    };
    let mut items = b.emit(Scope::Screen, el)?;
    if b.expr().is_some_and(|e| e.is_place()) {
        items = format!("&{items}");
    }
    let key = match bound(bindings, v::KEY) {
        Some(k) => k.emit(Scope::Template, el)?,
        None => INDEX.to_string(),
    };
    Ok(Source {
        attr,
        items,
        key,
        raw: b.raw.clone(),
    })
}

impl VmGen<'_> {
    /// Compiles list `el` (owner of its own actions): its template, its
    /// host field, its update and selection write-back. Its other bindings
    /// (bg, w, ...) are applied by the caller.
    pub(super) fn list(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        bindings: &[(String, Binding)],
        attrs: &HashMap<String, String>,
        spot: &Spot,
    ) -> Result<(), String> {
        self.n_lists += 1;
        let owner = self.n_lists;
        let src = source(el, bindings)?;
        let selected = bound(bindings, at::SELECTED);
        if selected.is_some() && el.name != y::LIST_VIEW {
            return err(el, "selected is for ListView");
        }
        let item_height = attrs
            .get(at::ITEM_HEIGHT)
            .map(|h| yui::number(el, at::ITEM_HEIGHT, h))
            .transpose()?;
        let templates = templates(el)?;
        let t = self.n_templates;
        self.n_templates += 1;
        let item_update = self.item_template(t, templates.item, owner, &src.items)?;
        let (empty_fn, empty_update) = self.empty_state(t, templates.empty)?;
        let ih = option(item_height.map(|h| format!("{h:?}")));
        let sc = option(spot.scroller.map(|s| s.to_string()));
        let _ = writeln!(
            self.parts,
            "        parts.list({owner}, {}, {sc}, {ih});",
            path_lit(spot.path)
        );
        let field = format!("l{owner}");
        let host = format!(
            "::yog_kit::items::ItemsHost::new({owner}, {}, {ih}, {sc}, {})",
            path_lit(spot.path),
            selected.is_some()
        );
        self.view_fields.push((
            field.clone(),
            format!("::yog_kit::items::ItemsHost<__T{t}>"),
            host,
        ));
        let sel = match selected {
            Some(b) => Some(b.emit(Scope::Screen, el)?),
            None => None,
        };
        let fill = Fill {
            file: &self.file,
            line: el.line,
            t,
            field: &field,
            src: &src,
            selected: sel,
            item_update,
            empty_fn,
            empty_update,
        };
        ctx.update.push_str(&fill.update());
        if let Some(b) = selected {
            self.select_back(el, b, owner, &src)?;
        }
        Ok(())
    }

    /// Item template `t`'s struct, with its slots; its update code.
    fn item_template(
        &mut self,
        t: usize,
        template: &Element,
        owner: u16,
        items: &str,
    ) -> Result<String, String> {
        let mut tctx = Ctx::new(Scope::Template, INSTANCE_FIELDS, ITEM_TREE, owner);
        let saved = std::mem::take(&mut self.n_slots);
        let tel = self.element(&mut tctx, template, &[], Some(items))?;
        self.n_slots = saved;
        let (texpr, ids) = yui::static_widget(&tel, false, 1, self.mod_id)?;
        self.absorb(ids);
        let fields: String = tctx
            .slots
            .iter()
            .map(|(n, ty)| format!("    {n}: ::yog_kit::bind::Slot<{ty}>,\n"))
            .collect();
        let inits: String = tctx
            .slots
            .iter()
            .map(|(n, _)| format!("{n}: ::yog_kit::bind::Slot::new(), "))
            .collect();
        // The template's struct is named by its number, and its build may not use the import.
        let _ = write!(
            self.templates,
            "/// Item template {t} ({}:{}).\n#[doc(hidden)]\n#[allow(non_camel_case_types)]\nstruct __T{t} {{\n{fields}}}\n\nimpl __T{t} {{\n    fn build() -> (Self, ::yog_api::ui::Widget) {{\n        #[allow(unused_imports)]\n        {UI_USE}\n        (Self {{ {inits}}}, {texpr})\n    }}\n}}\n\n",
            self.file, template.line
        );
        Ok(tctx.update)
    }

    /// The list's empty state: built once, bound in view scope. Its
    /// builder fn (as an `Option`), its update code.
    fn empty_state(
        &mut self,
        t: usize,
        empty: Option<&Element>,
    ) -> Result<(String, String), String> {
        let Some(e) = empty else {
            return Ok((option(None), String::new()));
        };
        let mut ectx = Ctx::new(Scope::Screen, VIEW_FIELDS, ITEM_TREE, SCREEN_OWNER);
        let eel = self.element(&mut ectx, e, &[], None)?;
        let (eexpr, ids) = yui::static_widget(&eel, false, 1, self.mod_id)?;
        self.absorb(ids);
        let _ = write!(
            self.templates,
            // The empty state's fn is named by its number, and may not use the import.
            "#[doc(hidden)]\n#[allow(non_snake_case)]\nfn __E{t}() -> ::yog_api::ui::Widget {{\n    #[allow(unused_imports)]\n    {UI_USE}\n    {eexpr}\n}}\n\n"
        );
        Ok((option(Some(format!("__E{t}"))), ectx.update))
    }

    /// The selection written back to the view model, when the binding does.
    fn select_back(
        &mut self,
        el: &Element,
        b: &Binding,
        owner: u16,
        src: &Source,
    ) -> Result<(), String> {
        let write = match b.write_back("__k", "::std::option::Option::Some(__k)")? {
            WriteBack::Stmt(w) => w,
            WriteBack::Nowhere => {
                return err(el, "selected Mode=TwoWay needs a field or BindBack=method");
            }
            WriteBack::OneWay => return Ok(()),
        };
        let (items, key) = (&src.items, &src.key);
        self.dispatch.push((
            format!("::yog_kit::ViewEvent::Action(::yog_api::ui::Action {{ owner: {owner}, slot: ::yog_kit::items::SELECT, index }})\n            | ::yog_kit::ViewEvent::Select {{ list: {owner}, index }}"),
            format!("let __k = ::std::iter::IntoIterator::into_iter({items}).enumerate().nth(index as usize).map(|(index, item)| {{ let _ = (&index, &item); ::std::clone::Clone::clone(&({key})) }});\n                if let ::std::option::Option::Some(__k) = __k {{ {write} }}"),
        ));
        Ok(())
    }

    /// Binds the component properties among `bindings` (removing them):
    /// one text slot each, and an update block that rebuilds the instance
    /// at `path` from `out` (its static attributes) plus the slots' values.
    /// Returns the marker the block leaves for the resets of the element's
    /// other bindings.
    pub(super) fn component_props(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        out: &Element,
        bindings: &mut Vec<(String, Binding)>,
        path: &[u16],
    ) -> Result<String, String> {
        let mut inst = out.clone();
        let mut sets = String::new();
        let (props, rest): (Vec<_>, Vec<_>) = std::mem::take(bindings)
            .into_iter()
            .partition(|(k, _)| !yui::is_common_attr(k));
        *bindings = rest;
        for (key, b) in props {
            if b.mode == Mode::TwoWay || b.bind_back.is_some() {
                return err(el, format!("{key}: component properties bind one way"));
            }
            let expr = b.rust(ctx.scope, el)?;
            let value = if b.has_opt() {
                let fallback = b.fallback.clone().unwrap_or_default();
                format!(
                    "(|| ::std::option::Option::Some(::yog_kit::bind::text(&({expr}))))().unwrap_or_else(|| ::std::string::String::from({fallback:?}))"
                )
            } else {
                format!("::yog_kit::bind::text(&({expr}))")
            };
            let slot = self.slot(ctx, "String");
            let f = format!("{}{slot}", ctx.fields);
            let _ = write!(
                sets,
                "        // @yui {}:{} {key}=\"{}\"\n        __props |= {f}.set({value});\n",
                self.file,
                el.line,
                b.raw.replace('\n', " ")
            );
            inst.attrs.push((key, format!("{RAW_VALUE}*{f}.get()")));
        }
        let (expr, _) = yui::static_widget(&inst, false, 2, self.mod_id)?;
        let marker = format!("\u{1}resets{}\u{1}", self.n_slots);
        let _ = write!(
            ctx.update,
            // The rebuilt instance's code may not use the import.
            "        let mut __props = false;\n{sets}        if __props {{\n            #[allow(unused_imports)]\n            {UI_USE}\n            if let ::std::option::Option::Some({WIDGET}) = {}.get_path_mut({}) {{ *{WIDGET} = {expr}; }}\n            u.relayout();{marker}\n        }}\n",
            ctx.tree,
            path_lit(path),
        );
        Ok(marker)
    }
}

/// What a list's update is made of.
struct Fill<'a> {
    file: &'a str,
    line: usize,
    /// The item template's number.
    t: usize,
    /// The view's host field.
    field: &'a str,
    src: &'a Source,
    /// The selected key's expression, when it's bound.
    selected: Option<String>,
    item_update: String,
    empty_fn: String,
    empty_update: String,
}

impl Fill<'_> {
    /// The list's update: its items through the host, each row's bindings,
    /// the empty state's when there are none.
    fn update(&self) -> String {
        let Fill {
            file,
            line,
            t,
            field,
            src,
            ..
        } = self;
        let (items, key) = (&src.items, &src.key);
        let mut u = format!(
            "        // @yui {file}:{line} {}=\"{}\"\n        {{\n",
            src.attr, src.raw
        );
        let sel = match &self.selected {
            Some(e) => {
                let _ = writeln!(u, "            let __sel = {e};");
                format!("::yog_kit::bind::is_selected(&__sel, &({key}))")
            }
            None => yog_ids::yui_value::FALSE.into(),
        };
        let _ = writeln!(
            u,
            "            if let ::std::option::Option::Some(mut __it) = self.{field}.begin(root, u) {{"
        );
        let _ = writeln!(
            u,
            "            for (index, item) in ::std::iter::IntoIterator::into_iter({items}).enumerate() {{"
        );
        u.push_str("                let _ = (&index, &item);\n");
        let next = format!("__it.next(::yog_kit::bind::key(&({key})), {sel}, __T{t}::build)");
        if self.item_update.is_empty() {
            let _ = writeln!(u, "                let _ = {next};");
        } else {
            let _ = writeln!(
                u,
                "                let ::std::option::Option::Some(__row) = {next} else {{ continue }};"
            );
            u.push_str("                let ::yog_kit::items::Item { inst, w, u } = __row;\n");
            indent_into(&mut u, &self.item_update);
        }
        u.push_str("            }\n");
        if self.empty_update.is_empty() {
            let _ = writeln!(u, "            __it.finish({});", self.empty_fn);
        } else {
            let _ = writeln!(
                u,
                "            if let ::std::option::Option::Some((w, u)) = __it.finish({}) {{",
                self.empty_fn
            );
            indent_into(&mut u, &self.empty_update);
            u.push_str("            }\n");
        }
        u.push_str("            }\n        }\n");
        u
    }
}

/// `code`'s lines into `u`, a level deeper.
fn indent_into(u: &mut String, code: &str) {
    for line in code.lines() {
        let _ = writeln!(u, "        {line}");
    }
}
