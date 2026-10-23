//! A bound attribute's update code, and a command's statement.

use std::fmt::Write as _;

use super::binding::{Binding, Bound};
use super::expr::{CX, Ex, Head, INDEX, Scope, VM};
use super::state::{Ctx, VmGen, path_lit};
use super::target::{VALUE_MARK, target};
use super::{NEW_VALUE, WIDGET};
use crate::ui::yui::{Element, err};
use yog_data::Diagnostic;

/// Where a bound element is: its kind, its path in the tree, the view's
/// text box and scroller it is (none in a template).
pub(super) struct Spot<'a> {
    pub(super) kind: &'a str,
    pub(super) path: &'a [u16],
    pub(super) text_box: Option<u16>,
    pub(super) scroller: Option<u16>,
}

/// What a command's arguments are called once cloned out of a template's item.
const ARG: &str = "__a";

/// `f(E)`'s value, converted: a `?.` chain inside a closure, falling back
/// to `fallback` — a `String`'s from its text — or the default.
fn optional(conv: &str, expr: &str, b: &Binding, ty: &str) -> String {
    let fallback = match (&b.fallback, ty) {
        (Some(f), "String") => format!("::std::string::String::from({f:?})"),
        (Some(f), _) => f.clone(),
        (None, _) => "::std::default::Default::default()".into(),
    };
    format!(
        "(|| ::std::option::Option::Some({}))().unwrap_or_else(|| {fallback})",
        conv.replace(VALUE_MARK, expr)
    )
}

impl VmGen<'_> {
    /// Converts and applies one bound attribute.
    pub(super) fn bind_attr(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        at: &Spot,
        key: &str,
        b: &Binding,
    ) -> Result<(), Diagnostic> {
        let Some(t) = target(at.kind, key) else {
            return err(el, format!("{key} can't be bound"));
        };
        let expr = b.rust(ctx.scope, el)?;
        let (ty, conv) = t.conversion();
        let value = if b.has_opt() {
            optional(conv, &expr, b, ty)
        } else {
            conv.replacen(VALUE_MARK, &expr, 1)
        };
        let slot = self.slot(ctx, ty);
        let f = format!("{}{slot}", ctx.fields);
        let apply = t
            .apply(at.text_box, at.scroller)
            .or_else(|why| err(el, why))?;
        // What sets the widget reaches it by its path, there while the tree is as built.
        let reach = if apply.contains(WIDGET) {
            format!(
                " && let ::std::option::Option::Some({WIDGET}) = {}.get_path_mut({})",
                ctx.tree,
                path_lit(at.path)
            )
        } else {
            String::new()
        };
        // The new value where the binding uses it, else just whether it changed.
        let changed = if apply.contains(NEW_VALUE) {
            format!("if let ::std::option::Option::Some({NEW_VALUE}) = {f}.changed({value})")
        } else {
            format!("if {f}.set({value})")
        };
        let _ = write!(
            ctx.update,
            "        // @yui {}:{} {key}=\"{}\"\n        {changed}{reach} {{ {apply} }}\n",
            self.file,
            el.line,
            b.raw.replace('\n', " ")
        );
        Ok(())
    }

    /// The command statement for `b` in `scope` (`items`: a template's list).
    pub(super) fn command(
        &self,
        el: &Element,
        b: &Binding,
        scope: Scope,
        items: Option<&str>,
    ) -> Result<String, Diagnostic> {
        let Bound::Expr(e) = &b.bound else {
            return err(el, "command needs {x:Bind method(...)}");
        };
        let line = el.line;
        match (scope, items) {
            (Scope::Screen, _) | (Scope::Template, None) => Ok(format!(
                "{};",
                e.emit(Scope::Screen, true).map_err(|m| m.at_line(line))?
            )),
            (Scope::Template, Some(items)) => template_command(el, e, items),
        }
    }
}

/// A command in an item template: `vm.method(args)` / `cx.method()`, the
/// item's arguments cloned out of the item first.
fn template_command(el: &Element, e: &Ex, items: &str) -> Result<String, Diagnostic> {
    let line = el.line;
    let Ex::Post(head, segs) = e else {
        return err(el, "command must be a call: vm.method(...) or cx.close()");
    };
    let rooted = matches!(head, Head::Ident(n, None) if n == VM || n == CX);
    let Some((last, before)) = segs
        .split_last()
        .filter(|(s, _)| s.args.is_some() && rooted)
    else {
        return err(
            el,
            "commands in item templates call the view model: write vm.method(...) (or cx....)",
        );
    };
    let args = last.args.clone().unwrap_or_default();
    let names: Vec<String> = (0..args.len()).map(|i| format!("{ARG}{i}")).collect();
    let mut called = last.clone();
    called.args = Some(
        names
            .iter()
            .map(|n| Ex::Post(Head::Lit(n.clone()), Vec::new()))
            .collect(),
    );
    let call = Ex::Post(
        head.clone(),
        before.iter().cloned().chain([called]).collect(),
    );
    let call = call
        .emit(Scope::Template, true)
        .map_err(|m| Diagnostic::from(format!("{m}")).at_line(line))?;
    if args.is_empty() {
        return Ok(format!("let _ = {INDEX}; {call};"));
    }
    let cloned = args
        .iter()
        .map(|a| {
            a.emit(Scope::Template, false)
                .map(|s| format!("::std::clone::Clone::clone(&({s}))"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|m| Diagnostic::from(format!("{m}")).at_line(line))?;
    Ok(format!(
        "let __args = ::std::iter::IntoIterator::into_iter({items}).enumerate().nth(index as usize).map(|(index, item)| {{ let _ = (&index, &item); ({},) }});\n                if let ::std::option::Option::Some(({},)) = __args {{ {call}; }}",
        cloned.join(", "),
        names.join(", ")
    ))
}
