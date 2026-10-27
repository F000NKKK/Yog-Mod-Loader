//! `ref` and `out`: a parameter is a cell (`Ref<T>`) the method reads and
//! writes through (`x.value`); an argument a cell made of its place's value
//! (none for `out`), the place given the cell's value back after the call
//! (`Ys.before(F($t1 = new Ref<>(a)), a = $t1.value)`). The borrow checker
//! lets nothing else reach the place meanwhile, so a copy in and out is the
//! place itself. A method the runtime writes by hand takes the value and
//! gives the new one back instead (`calls.rs`). What's read of the place — its object, its index — is
//! read once, into temporaries declared before the statement.

use std::collections::HashSet;

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, ExprKind, Param, Pass};
use yog_script_ids::java::NEW;
use yog_script_ids::jvm::{BEFORE, HELPERS, REF, REF_VALUE};

use super::Gen;
use crate::types::RUNTIME;

/// The `ref` and `out` ones of `params`, by name.
pub(crate) fn cells(params: &[Param]) -> HashSet<String> {
    params
        .iter()
        .filter(|p| matches!(p.pass, Some(Pass::Ref | Pass::Out)))
        .map(|p| p.name.text.clone())
        .collect()
}

impl Gen<'_> {
    /// Call `e`, written by `write`, then its `ref` and `out` arguments'
    /// places given their values back: after its value, or — of a call
    /// giving none — as statements of their own.
    pub(crate) fn giving_back(
        &mut self,
        e: &Expr,
        write: impl FnOnce(&mut Self) -> String,
    ) -> String {
        let outer = std::mem::take(&mut self.given_back);
        let runtime = self.calls_runtime(e);
        let outer_runtime = std::mem::replace(&mut self.runtime_call, runtime);
        let call = write(self);
        self.runtime_call = outer_runtime;
        let back = std::mem::replace(&mut self.given_back, outer);
        if back.is_empty() {
            return call;
        }
        if matches!(self.types.of(e), Some(Type::Void) | None) {
            self.code.line(format!("{call};"));
            for b in back {
                self.code.line(format!("{b};"));
            }
            return String::new();
        }
        back.into_iter().fold(call, |v, b| {
            format!("{RUNTIME}.{HELPERS}.{BEFORE}({v}, {b})")
        })
    }

    /// Argument `place` passed by `ref` or `out`: a new cell, its place
    /// given back after the call.
    pub(crate) fn cell_arg(&mut self, place: &Expr, pass: Pass) -> String {
        let t = self.types.of(place).map_or_else(
            || yog_script_ids::jvm::OBJECT.to_string(),
            |t| self.java(t, true),
        );
        let cell = self.temp();
        self.code.line(format!("{REF}<{t}> {cell};"));
        let Some((read, written)) = self.place_of(place) else {
            self.not_yet("a `ref` or `out` of this", place.span);
            return String::new();
        };
        self.given_back
            .push(format!("{written} = {cell}.{REF_VALUE}"));
        match pass {
            Pass::Out => format!("{cell} = {NEW} {REF}<>()"),
            _ => format!("{cell} = {NEW} {REF}<>({read})"),
        }
    }

    /// A variable's place as read (its parts read once, into temporaries)
    /// and as written after: a local (`out let n` declared before the
    /// statement), a field, an array's item.
    fn place_of(&mut self, place: &Expr) -> Option<(String, String)> {
        match &place.kind {
            ExprKind::Declare { name, .. } => {
                let t = self.types.of(place).cloned()?;
                let n = Self::ident(&name.text);
                let declared = self.java(&t, false);
                self.code.line(format!("{declared} {n};"));
                // Read by none: an `out`'s place is only written.
                Some((n.clone(), n))
            }
            ExprKind::Ident(_) => {
                let v = self.expr(place);
                Some((v.clone(), v))
            }
            ExprKind::Member {
                expr,
                name,
                safe: false,
            } => {
                if let Some(Type::Own(of, args)) = self.types.of(expr).map(Type::non_null)
                    && self.index.called(&(of.clone(), args.len()), &name.text)
                {
                    return None;
                }
                let (first, then) = self.read_once(expr);
                let n = Self::ident(&name.text);
                Some((format!("{first}.{n}"), format!("{then}.{n}")))
            }
            ExprKind::Index { expr, args, .. }
                if matches!(
                    self.types.of(expr).map(Type::non_null),
                    Some(Type::Array(..))
                ) =>
            {
                let (first, then) = self.read_once(expr);
                let (mut read, mut written) = (vec![], vec![]);
                for a in args {
                    let (r, w) = self.read_once(a);
                    read.push(r);
                    written.push(w);
                }
                Some((
                    Self::array_slot(&first, &read),
                    Self::array_slot(&then, &written),
                ))
            }
            _ => None,
        }
    }

    /// `e` read where it's first met and named after: a name or `this` as
    /// it is, else into a temporary declared before the statement
    /// (`($t2 = Make())` first, `$t2` then).
    fn read_once(&mut self, e: &Expr) -> (String, String) {
        let v = self.expr(e);
        if matches!(
            e.kind,
            ExprKind::Ident(_) | ExprKind::This | ExprKind::Int(_)
        ) {
            return (v.clone(), v);
        }
        let t = self.types.of(e).map_or_else(
            || yog_script_ids::jvm::OBJECT.to_string(),
            |t| self.java(t, false),
        );
        let temp = self.temp();
        self.code.line(format!("{t} {temp};"));
        (format!("({temp} = {v})"), temp)
    }
}
