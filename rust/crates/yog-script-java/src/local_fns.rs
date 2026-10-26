//! Local functions (C#'s) on the JVM: a local class holding them, made in
//! the block — at its start, so they're called before they're declared
//! too; where declared, when one reaches a local of the block itself (it
//! isn't there yet at the start). Its methods reach the block's locals and
//! the object as a lambda's code does (`Outer.this`).

use yog_script_analyze::ast::{Decl, DeclKind, Expr, ExprKind, Stmt};
use yog_script_ids::java::{CLASS, FINAL, NEW, VAR};

use crate::Gen;
use crate::decls::Place;

/// What a local function's class is named after.
const CLASS_PREFIX: &str = "$Local";

impl Gen<'_> {
    /// The local functions of `stmts` (a block) reaching none of its own
    /// locals, written at its start.
    pub(crate) fn local_functions_first(&mut self, stmts: &[Stmt]) {
        let first: Vec<&Decl> = local_decls(stmts)
            .filter(|(at, d)| !reaches_block_locals(d, &stmts[..*at]))
            .map(|(_, d)| d)
            .collect();
        if !first.is_empty() {
            self.local_class(&first);
        }
    }

    /// Local function `d` where it's declared: written, unless it was at
    /// its block's start.
    pub(crate) fn local_function(&mut self, d: &Decl) {
        if self.local_fn_holder(fn_name(d)).is_none() {
            self.local_class(&[d]);
        }
    }

    /// Where local function `name` is called through, if it's one in reach.
    pub(crate) fn local_fn_holder(&self, name: &str) -> Option<&str> {
        self.local_fns
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, h)| h.as_str())
    }

    /// `functions` in a local class, and the object of it they're called on.
    fn local_class(&mut self, functions: &[&Decl]) {
        let class = format!("{CLASS_PREFIX}{}", self.temp());
        let holder = self.temp();
        // Its methods' `this` is the object's, outside it.
        let this = self
            .owner
            .as_ref()
            .filter(|_| !self.statik)
            .map(|(name, arity)| self.index.place(name, *arity));
        let outer = match this {
            Some(t) if self.this_name.is_none() => self.this_name.replace(t),
            _ => self.this_name.clone(),
        };
        self.code.open(format!("{FINAL} {CLASS} {class}"));
        // In the class, its functions call each other directly.
        let depth = self.local_fns.len();
        for d in functions {
            self.local_fns.push((fn_name(d).to_string(), String::new()));
        }
        for d in functions {
            self.decl(d, Place::Class);
        }
        self.local_fns.truncate(depth);
        for d in functions {
            self.local_fns
                .push((fn_name(d).to_string(), holder.clone()));
        }
        self.code.close("");
        self.this_name = outer;
        self.code
            .line(format!("{FINAL} {VAR} {holder} = {NEW} {class}();"));
    }
}

/// The local functions `stmts` declare, each with where.
fn local_decls(stmts: &[Stmt]) -> impl Iterator<Item = (usize, &Decl)> {
    stmts.iter().enumerate().filter_map(|(at, s)| match s {
        Stmt::Function(d) => Some((at, &**d)),
        _ => None,
    })
}

/// Local function `d`'s name.
fn fn_name(d: &Decl) -> &str {
    match &d.kind {
        DeclKind::Method(m) => &m.name.text,
        _ => "",
    }
}

/// Whether local function `d` reaches a local `before` declares (its block's own).
fn reaches_block_locals(d: &Decl, before: &[Stmt]) -> bool {
    let DeclKind::Method(m) = &d.kind else {
        return false;
    };
    let names: Vec<&str> = before
        .iter()
        .flat_map(|s| match s {
            Stmt::Local { binding, .. } => binding.names().collect(),
            _ => Vec::new(),
        })
        .collect();
    if names.is_empty() {
        return false;
    }
    let mut found = false;
    crate::drops::each_body_expr(m.body.as_ref(), &mut |e| found |= names_any(e, &names));
    found
}

/// Whether `e` names one of `names`.
fn names_any(e: &Expr, names: &[&str]) -> bool {
    if let ExprKind::Ident(n) = &e.kind
        && names.contains(&n.as_str())
    {
        return true;
    }
    let mut found = false;
    e.each_child(|c| found |= names_any(c, names));
    found
}
