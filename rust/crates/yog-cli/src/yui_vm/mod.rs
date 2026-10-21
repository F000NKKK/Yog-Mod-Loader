//! View-model screens: a `.yui` whose `<Screen>` has `x:ViewModel="MyVm"`
//! compiles into a `yog_kit::View` — the static widget tree (built by
//! [`crate::yui`]'s generator), compiled `{x:Bind ...}` bindings patching
//! it, commands, two-way text boxes and toggles, lists with item templates,
//! and the `register`/`open`/`close` functions. See `docs/mvvm.md` in
//! Yog-IDLE for the format.

mod bind;
mod binding;
mod element;
mod expr;
mod list;
mod state;
mod target;

use std::fmt::Write as _;

pub(crate) use binding::compile_bind;

use crate::rust_code::{f32_lit, fill};
use crate::yui::{self, Element, UI_USE, err};
use expr::Scope;
use state::{Ctx, SCREEN_OWNER, VIEW_FIELDS, VIEW_TREE, VmGen};

/// The codegen version checked by `yog_kit::__codegen::require`.
use yog_ids::versions::UI_CODEGEN as CODEGEN;

/// What a binding's apply code calls the new value.
const NEW_VALUE: &str = "__v";
/// What generated code calls the widget a binding sets.
const WIDGET: &str = "__w";

/// The generated code of a view-model screen.
const TEMPLATE: &str = include_str!("view.g.rs.in");

/// What a view-model screen's root says: its id, view model, how it's made.
struct Root {
    id: String,
    vm: String,
    create: String,
}

/// The root's id and view model (and how the view model is made: its
/// `x:Create`, else its `Default`).
fn read_root(root: &Element) -> Result<Root, String> {
    if root.name != yog_ids::yui::SCREEN {
        return err(root, "a view-model screen's root must be <Screen>");
    }
    let id = root
        .attr(yog_ids::yui_attr::ID)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "line {}: <Screen> needs an id (the UI id, e.g. \"mymod:main\")",
                root.line
            )
        })?
        .to_string();
    let vm = root
        .attr(yog_ids::yui_attr::X_VIEW_MODEL)
        .unwrap_or_default()
        .trim()
        .to_string();
    if vm.is_empty() {
        return err(root, "x:ViewModel names the view-model type");
    }
    let create = root.attr(yog_ids::yui_attr::X_CREATE).map_or_else(
        || format!("<{vm} as ::std::default::Default>::default()"),
        str::to_string,
    );
    Ok(Root { id, vm, create })
}

/// Generates a view-model screen's `.g.rs` (see the module docs).
pub fn generate(
    root: &Element,
    name: &str,
    mod_id: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    let Root { id, vm, create } = read_root(root)?;
    let frame = yui::screen_frame(root)?;
    let mut g = VmGen::new(format!("{name}.yui"), mod_id, yui::screen_theme(root)?);
    for c in root
        .children
        .iter()
        .filter(|c| c.name == yog_ids::yui_vm::SCREEN_RESOURCES)
    {
        g.collect_resources(c)?;
    }
    let mut content = root.clone();
    content.attrs.retain(|(k, _)| k != yog_ids::yui_attr::ID);
    let mut ctx = Ctx::new(Scope::Screen, VIEW_FIELDS, VIEW_TREE, SCREEN_OWNER);
    let tree = g.element(&mut ctx, &content, &[], None)?;
    let (expr, ids) = yui::static_widget(&tree, false, 0, mod_id)?;
    g.absorb(ids);
    let mut view_fields = String::new();
    let mut view_inits = String::new();
    for (n, ty, init) in &g.view_fields {
        let _ = writeln!(view_fields, "    {n}: {ty},");
        let _ = writeln!(view_inits, "            {n}: {init},");
    }
    let dispatch: String = g
        .dispatch
        .iter()
        .map(|(pat, body)| {
            format!("            {pat} => {{\n                {body}\n            }}\n")
        })
        .collect();
    let values = [
        ("NAME", name.to_string()),
        ("VM", vm),
        ("CODEGEN", CODEGEN.to_string()),
        ("ID_LIT", format!("{id:?}")),
        ("WIDTH", f32_lit(frame.width)),
        ("HEIGHT", f32_lit(frame.height)),
        ("IDS", yui::str_consts(g.ids.as_slice())),
        ("TEMPLATES", std::mem::take(&mut g.templates)),
        ("VIEW_FIELDS", view_fields),
        ("ANCHOR", yui::pascal(frame.anchor)),
        ("MODAL", frame.modal.to_string()),
        ("PAUSE", frame.pause.to_string()),
        ("LIVE", frame.live.to_string()),
        ("UI_USE", UI_USE.to_string()),
        ("EXPR", expr),
        ("PARTS", std::mem::take(&mut g.parts)),
        ("VIEW_INITS", view_inits),
        ("UPDATE", ctx.update),
        ("DISPATCH", dispatch),
        ("CREATE", create),
    ];
    Ok((fill(TEMPLATE, &values), Vec::new()))
}

#[cfg(test)]
mod tests;
