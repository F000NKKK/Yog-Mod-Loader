//! Patterns: the names they bind, what they compare with.

use super::{DefKind, Resolver};
use crate::ast::Pattern;

impl Resolver<'_, '_> {
    /// The names a pattern binds, bound here; what it compares with, looked up.
    pub(super) fn bind_pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Wild(_) => {}
            Pattern::Value(e) => self.expr(e),
            Pattern::Let(n) => self.bind(n, DefKind::Variable),
            Pattern::Type { ty, bind } => {
                self.ty(ty);
                if let Some(n) = bind {
                    self.bind(n, DefKind::Variable);
                }
            }
            Pattern::Variant { path, args } => {
                if let Some(first) = path.first() {
                    self.refer(&first.text, first.span);
                }
                args.iter().for_each(|a| self.bind_pattern(a));
            }
            Pattern::Relational { value, .. } => self.expr(value),
            Pattern::Range { from, to } => {
                self.expr(from);
                self.expr(to);
            }
            Pattern::Tuple(ps) | Pattern::List(ps, _) => ps.iter().for_each(|a| self.bind_pattern(a)),
            Pattern::Slice(inner) => {
                if let Some(p) = inner {
                    self.bind_pattern(p);
                }
            }
            Pattern::Property { ty, fields } => {
                self.opt_ty(ty.as_ref());
                fields.iter().for_each(|(_, p)| self.bind_pattern(p));
            }
            Pattern::Or(a, b) | Pattern::And(a, b) => {
                self.bind_pattern(a);
                self.bind_pattern(b);
            }
            Pattern::Not(a) => self.bind_pattern(a),
        }
    }
}
