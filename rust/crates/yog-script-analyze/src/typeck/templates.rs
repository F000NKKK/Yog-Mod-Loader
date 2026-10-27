//! A string written for a message template parameter
//! (`[StructuredMessageTemplate(nameof(message))]`, `logging`): the template
//! as written — its holes names, its values the arguments after it
//! (`"Moved {Count} items", count`); or, none given after it, its holes the
//! values too (`"Moved {count} items"`: `count`, named `count`). Its holes
//! aren't checked until the call's method is known: names aren't code.

use super::Checker;
use super::members::Callee;
use super::own::TypeKey;
use super::types::MethodUse;
use crate::ast::{Arg, Expr, ExprKind, StrSeg};
use yog_ids::script_problem;
use yog_script_ids::system::{LOGGER_EXTENSIONS, LOGGER_MESSAGE};
use crate::lexer::VERBATIM;
use crate::lexer::string::{CLOSE, ESCAPE, ESCAPES, OPEN, QUOTE};

/// A string written for a message template: its text as written (holes
/// and all), and whether its holes are its values.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageTemplate {
    pub text: String,
    pub values: bool,
}

impl Checker<'_, '_, '_> {
    /// The written arguments a method `target` (or an extension of its
    /// name) may take as a message template: strings with holes, there.
    pub(super) fn template_slots(&self, target: &Callee, callee: &Expr, args: &[Arg]) -> Vec<usize> {
        let ExprKind::Member { name, .. } = &callee.kind else {
            return Vec::new();
        };
        let mut slots: Vec<usize> = Vec::new();
        let mut note = |at: Option<usize>| {
            if let Some(i) = at
                && args.get(i).is_some_and(|a| has_holes(&a.value))
                && !slots.contains(&i)
            {
                slots.push(i);
            }
        };
        let instance = matches!(target, Callee::Own(_));
        if let Callee::Own(methods) | Callee::Extension(methods) = target {
            for (m, _) in methods {
                note(self.template_of(m, &name.text).map(|p| p - usize::from(!instance)));
            }
        }
        for (_, m) in self.own.extension_methods.get(&name.text).into_iter().flatten() {
            note(self.template_of(m, &name.text).and_then(|p| p.checked_sub(1)));
        }
        slots
    }

    /// The index of method `m`'s (named `name`) template parameter, if it has one.
    fn template_of(&self, m: &crate::ast::Method, name: &str) -> Option<usize> {
        let param = self
            .own
            .all()
            .find_map(|(_, t)| {
                let methods = t.methods.get(name)?;
                let at = methods.iter().position(|x| x.name.span == m.name.span)?;
                t.templates.get(name)?.get(at).cloned().flatten()
            })?;
        m.params.iter().position(|p| p.name.text == param)
    }

    /// The method chosen (`used`, if one): each of `slots` it takes as a
    /// template noted so (its holes checked when they're its values); the
    /// others checked as the strings they are.
    pub(super) fn templates_chosen(&mut self, used: Option<&MethodUse>, written: &[Arg], slots: &[usize]) {
        let chosen = used.and_then(|u| self.chosen_template(u, written.len()));
        if let Some(u) = used
            && u.owner.as_deref() == Some(LOGGER_EXTENSIONS)
            // Its own methods write through each other.
            && !matches!(&self.this, Some(super::model::Type::Own(n, _)) if n == LOGGER_EXTENSIONS)
            && let Some(at) = written.first()
        {
            self.report.warning(
                script_problem::LOG_BY_METHOD,
                at.value.span,
                format!("`{}` reads its template and boxes its values at each entry: a `[{LOGGER_MESSAGE}]` method checks the level first and formats by compiled code", u.name),
            );
        }
        for &i in slots {
            let arg = &written[i].value;
            match chosen {
                Some((at, values)) if at == i => {
                    let text = template_text(self.report.text(arg.span));
                    if values {
                        for h in holes(arg) {
                            self.expr(h);
                        }
                    }
                    self.types.set_template(arg, MessageTemplate { text, values });
                }
                _ => {
                    self.expr(arg);
                }
            }
        }
    }

    /// The written index of `used`'s template parameter, and whether a
    /// template written there gives the values (nothing written after it,
    /// a `params` one after it).
    fn chosen_template(&self, used: &MethodUse, written: usize) -> Option<(usize, bool)> {
        let owner = used.owner.as_ref()?;
        let own = self.own.of_key(&TypeKey::new(owner, used.owner_arity))?;
        let m = own.methods.get(&used.name)?.get(used.overload)?;
        let param = own.templates.get(&used.name)?.get(used.overload)?.as_ref()?;
        let p = m.params.iter().position(|x| &x.name.text == param)?;
        let at = p - usize::from(used.extension);
        let rest_after = m.params.len() == p + 2 && m.params.last().is_some_and(|x| x.params);
        Some((at, rest_after && written == at + 1))
    }
}

/// Whether `e` is a string written with holes.
fn has_holes(e: &Expr) -> bool {
    matches!(&e.kind, ExprKind::Str(segs) if segs.iter().any(|s| matches!(s, StrSeg::Expr(_))))
}

/// Each hole's value of string `e`, in order.
pub fn holes(e: &Expr) -> Vec<&Expr> {
    let ExprKind::Str(segs) = &e.kind else {
        return Vec::new();
    };
    segs.iter()
        .filter_map(|s| match s {
            StrSeg::Expr(h) => Some(hole_value(h)),
            StrSeg::Text(_) => None,
        })
        .collect()
}

/// A hole's value without what its alignment and format made of it
/// (`{x,5:F2}` is `x.ToString("F2").PadLeft(5)`: `x`; the template keeps them).
fn hole_value(h: &Expr) -> &Expr {
    use yog_script_ids::members::{PAD_LEFT, PAD_RIGHT, TO_STRING};
    let ExprKind::Call { callee, args } = &h.kind else {
        return h;
    };
    let ExprKind::Member { expr, name, .. } = &callee.kind else {
        return h;
    };
    let made = (h.span == expr.span) && h.span == callee.span;
    match name.text.as_str() {
        PAD_LEFT | PAD_RIGHT if made => hole_value(expr),
        TO_STRING if made && args.len() == 1 => expr,
        _ => h,
    }
}

/// A string literal's text as written (`"…"`), its escapes read, its holes
/// kept: what a template is (a brace escaped, doubled as the template has it).
fn template_text(written: &str) -> String {
    let verbatim = written.strip_prefix(VERBATIM);
    let quoted = verbatim.unwrap_or(written);
    let inner = quoted.strip_prefix(QUOTE).and_then(|s| s.strip_suffix(QUOTE)).unwrap_or(quoted);
    // `@"…"`: no escapes, a `""` its quote.
    if verbatim.is_some() {
        return inner.replace(&format!("{QUOTE}{QUOTE}"), &QUOTE.to_string());
    }
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != ESCAPE {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(b @ (OPEN | CLOSE)) => {
                out.push(b);
                out.push(b);
            }
            Some(e) => out.push(ESCAPES.iter().find(|(k, _)| *k == e).map_or(e, |(_, v)| *v)),
            None => {}
        }
    }
    out
}
