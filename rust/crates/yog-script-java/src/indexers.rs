//! An indexer as Java writes it: its accessors are methods named as the
//! CLR names them (`get_Item`, `set_Item`, `ast::accessor_name`), so `x[i]`
//! is a call of one and `x[i] = v` of the other.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, Expr, ExprKind, Indexer, UnOp, accessor_name};
use yog_script_analyze::words::{get, set};

use super::Gen;
use crate::decls::Place;
use crate::types::RUNTIME;
use yog_script_ids::java::{NULL, VAR};
use yog_script_ids::jvm::{HELPERS, LENGTH_OF, OBJECT, SLICE};

impl Gen<'_> {
    /// An indexer declared: its accessors as methods.
    pub(crate) fn indexer(&mut self, d: &Decl, ix: &Indexer, place: Place) {
        for m in ix.getter().into_iter().chain(ix.setter()) {
            self.method(d, m, place);
        }
    }

    /// Whether `on` is indexed by its type's own indexer (not a list's or a
    /// map's built-in indexing).
    pub(crate) fn has_indexer(&self, on: &Expr) -> bool {
        matches!(self.types.of(on).map(Type::non_null), Some(Type::Own(..)))
    }

    /// `on[at]`, `on` already written: its getter called.
    pub(crate) fn index_read(on: &str, at: &str) -> String {
        format!("{on}.{}({at})", accessor_name(get::WORD.text))
    }

    /// `on[at] = v`, `on` already written: its setter called.
    pub(crate) fn index_write(on: &str, at: &str, v: &str) -> String {
        format!("{on}.{}({at}, {v});", accessor_name(set::WORD.text))
    }

    /// `on[args]` (`e`): an array's slot, an indexer's getter, a built-in's
    /// companion's, a list's or a map's item; `safe` (`on?[…]`): `null`
    /// when `on` is.
    pub(crate) fn index_expr(&mut self, e: &Expr, on: &Expr, args: &[Expr], safe: bool) -> String {
        let mut target = self.expr(on);
        // Held once when it's read again: `?[`'s test, `^n`'s length.
        let counted = args.iter().any(counts_back);
        let held = (safe || counted).then(|| {
            let t = self.temp();
            self.code.line(format!("{VAR} {t} = {target};"));
            t
        });
        if let Some(t) = &held {
            target.clone_from(t);
        }
        let outer = self.counted.replace(target.clone());
        let read = self.index_of(e, on, &target, args);
        self.counted = outer;
        match held.filter(|_| safe) {
            Some(t) => format!("({t} == {NULL} ? {NULL} : {read})"),
            None => read,
        }
    }

    /// `^n`: the length of what's indexed, less `n`.
    pub(crate) fn counted_back(&mut self, n: &Expr) -> String {
        let Some(of) = self.counted.clone() else {
            self.not_yet("`^n` outside an index", n.span);
            return String::new();
        };
        let n = self.expr(n);
        format!("({RUNTIME}.{HELPERS}.{LENGTH_OF}({of}) - {n})")
    }

    fn index_of(&mut self, e: &Expr, on: &Expr, target: &str, args: &[Expr]) -> String {
        if let [
            Expr {
                kind:
                    ExprKind::Range {
                        from,
                        to,
                        inclusive,
                    },
                ..
            },
        ] = args
        {
            let (a, b) = (self.expr(from), self.expr(to));
            let t = self
                .types
                .of(e)
                .map_or_else(|| OBJECT.to_string(), |t| self.java(t, false));
            return format!("(({t}) {RUNTIME}.{HELPERS}.{SLICE}({target}, {a}, {b}, {inclusive}))");
        }
        let at: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        if let Some(Type::Array(..)) = self.types.of(on).map(Type::non_null) {
            return Self::array_slot(target, &at);
        }
        if self.has_indexer(on) {
            return Self::index_read(target, &at.join(", "));
        }
        if let Some(host) = self.types.of(on).and_then(|t| self.index.host(t)) {
            let all: Vec<String> = std::iter::once(target.to_string()).chain(at).collect();
            return Self::index_read(&host, &all.join(", "));
        }
        let at = at.join(", ");
        format!(
            "{RUNTIME}.{HELPERS}.at({target}, {at}, {})",
            self.at(e.span)
        )
    }
}

/// Whether index argument `e` counts back from the end (`^n`, in a range too).
fn counts_back(e: &Expr) -> bool {
    if matches!(
        e.kind,
        ExprKind::Unary {
            op: UnOp::FromEnd,
            ..
        }
    ) {
        return true;
    }
    let mut found = false;
    e.each_child(|c| found |= counts_back(c));
    found
}
