//! Patterns, as C#'s, in Java: each a condition (`v instanceof
//! Mode.Charging $c && $c.Rate() > 3`), its cases' and types' objects
//! bound as Java's `instanceof` binds them. In an `is` a name it binds
//! (`let r`) is declared before its statement — its scope the block, as
//! C#'s — and assigned in the condition (`Ys.bound(r = $c.Rate())`); a
//! `switch`'s arm declares them inside its `if`s.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, ExprKind, Name, Pattern, TypeKind};
use yog_script_ids::clr::TUPLE_ITEM;
use yog_script_ids::jvm::enums::COMPARE;
use yog_script_ids::jvm::{BOUND, EQUALS, HELPERS, OBJECTS, RUNTIME};
use yog_script_ids::system::ENUM;

use super::Gen;
use crate::index::Kind;

/// What's always so: an arm taking anything.
const ALWAYS: &str = "true";

impl Gen<'_> {
    /// `x is pattern` as a value: its condition, the names it binds
    /// declared before its statement.
    pub(crate) fn is(&mut self, on: &Expr, p: &Pattern) -> String {
        let matched = self.types.of(on).cloned();
        let v = self.expr(on);
        // What's matched read once: a name as it is, else bound (`instanceof`).
        let (v, bound) = match (&on.kind, matched.as_ref()) {
            (ExprKind::Ident(_) | ExprKind::This, _) | (_, None) => (v, String::new()),
            (_, Some(t)) => {
                let (temp, boxed) = (self.temp(), self.java(t, true));
                let bound = format!("((Object) ({v})) instanceof {boxed} {temp} && ");
                (temp, bound)
            }
        };
        match self.test(p, &v, matched.as_ref()) {
            Some(cond) => format!("({bound}{cond})"),
            None => String::new(),
        }
    }

    /// Pattern `p` on `v` (of type `on`, when known) as a condition, the
    /// names it binds declared before the statement it's in.
    fn test(&mut self, p: &Pattern, v: &str, on: Option<&Type>) -> Option<String> {
        Some(match p {
            Pattern::Wild(_) => ALWAYS.to_string(),
            Pattern::Let(n) => self.bound(n, v),
            Pattern::Value(x) if matches!(x.kind, ExprKind::Null) => format!("{v} == null"),
            Pattern::Value(x) => {
                let x = self.expr(x);
                format!("{OBJECTS}.{EQUALS}({v}, {x})")
            }
            Pattern::Type { ty, bind } => {
                self.type_test(ty, bind.as_ref().map(|n| n.text.as_str()), v)?
            }
            Pattern::Variant { path, args } => {
                let (case, fields) = self.case_test(path, v, on);
                let mut all = vec![case.test];
                for (arg, field) in args.iter().zip(fields) {
                    all.push(self.test(arg, &format!("{}.{field}()", case.bound), None)?);
                }
                all.join(" && ")
            }
            Pattern::Relational { op, value } => {
                let x = self.expr(value);
                let op = crate::exprs::op_text(*op);
                if self.is_enum(on) {
                    format!("{}.{COMPARE}({v}, {x}) {op} 0", self.index.place(ENUM, 0))
                } else {
                    format!("{v} {op} {x}")
                }
            }
            Pattern::Range { from, to } => {
                let (a, b) = (self.expr(from), self.expr(to));
                format!("{a} <= {v} && {v} <= {b}")
            }
            Pattern::Tuple(items) => {
                let mut all = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let t = tuple_item(on, i);
                    all.push(self.test(item, &format!("{v}.{TUPLE_ITEM}{}", i + 1), t.as_ref())?);
                }
                all.join(" && ")
            }
            Pattern::Property { ty, fields } => {
                let (mut all, of, target) = self.property_target(ty.as_ref(), v, on);
                for (name, field) in fields {
                    let (read, t) = self.member_read(&target, of.as_ref(), &name.text);
                    all.push(self.test(field, &read, t.as_ref())?);
                }
                all.join(" && ")
            }
            Pattern::Or(a, b) => format!("({} || {})", self.test(a, v, on)?, self.test(b, v, on)?),
            Pattern::And(a, b) => format!("({} && {})", self.test(a, v, on)?, self.test(b, v, on)?),
            Pattern::Not(inner) => format!("!({})", self.test(inner, v, on)?),
        })
    }

    /// The `if`s pattern `p` on `v` (of type `on`) opens, its names bound
    /// inside: how many to close.
    pub(crate) fn pattern_open(&mut self, p: &Pattern, (v, on): (&str, Option<&Type>)) -> usize {
        if !binds(p) {
            return match self.test(p, v, on) {
                Some(cond) if cond == ALWAYS => 0,
                Some(cond) => {
                    self.code.open(format!("if ({cond})"));
                    1
                }
                None => 0,
            };
        }
        match p {
            Pattern::Let(n) => {
                self.code
                    .line(format!("var {} = {v};", Self::ident(&n.text)));
                0
            }
            Pattern::Variant { path, args } => {
                let (case, fields) = self.case_test(path, v, on);
                self.code.open(format!("if ({})", case.test));
                let mut depth = 1;
                for (arg, field) in args.iter().zip(fields) {
                    depth += self.pattern_open(arg, (&format!("{}.{field}()", case.bound), None));
                }
                depth
            }
            Pattern::Tuple(items) => {
                let mut depth = 0;
                for (i, item) in items.iter().enumerate() {
                    let t = tuple_item(on, i);
                    depth += self
                        .pattern_open(item, (&format!("{v}.{TUPLE_ITEM}{}", i + 1), t.as_ref()));
                }
                depth
            }
            Pattern::Property { ty, fields } => {
                let (tests, of, target) = self.property_target(ty.as_ref(), v, on);
                let mut depth = 0;
                if !tests.is_empty() {
                    self.code.open(format!("if ({})", tests.join(" && ")));
                    depth += 1;
                }
                for (name, field) in fields {
                    let (read, t) = self.member_read(&target, of.as_ref(), &name.text);
                    depth += self.pattern_open(field, (&read, t.as_ref()));
                }
                depth
            }
            Pattern::And(a, b) => self.pattern_open(a, (v, on)) + self.pattern_open(b, (v, on)),
            _ => {
                self.not_yet(
                    "names bound inside `or` or `not`",
                    yog_script_analyze::token::Span::default(),
                );
                0
            }
        }
    }

    /// `let n` on `v` in a condition: `n` declared before its statement (of
    /// its type, a pattern's binding's), assigned as the condition goes.
    fn bound(&mut self, n: &Name, v: &str) -> String {
        let t = self
            .types
            .binding(n)
            .filter(|t| t.known())
            .cloned()
            .map_or_else(
                || yog_script_ids::jvm::OBJECT.to_string(),
                |t| self.java(&t, false),
            );
        let name = Self::ident(&n.text);
        self.code.line(format!("{t} {name};"));
        format!("{RUNTIME}.{HELPERS}.{BOUND}({name} = {v})")
    }

    fn is_enum(&self, on: Option<&Type>) -> bool {
        matches!(on.map(Type::non_null), Some(Type::Own(name, args))
            if self.index.kind(&(name.clone(), args.len())) == Some(Kind::Enum))
    }
}

impl Gen<'_> {
    /// Member `name` of `target` (of type `of`, when known) as a pattern
    /// reads it; its type isn't known here (a name bound in it is `var`).
    fn member_read(
        &mut self,
        target: &str,
        of: Option<&Type>,
        name: &str,
    ) -> (String, Option<Type>) {
        (self.member_of(target, of, name, false), None)
    }

    /// Written type `t` as far as reading its members needs: its name and
    /// arity, a built-in as itself.
    fn types_of_written(&self, t: &yog_script_analyze::ast::Type) -> Option<Type> {
        let TypeKind::Path { path, args } = &t.kind else {
            return None;
        };
        let name = path.last()?.text.clone();
        Some(
            yog_script_analyze::builtin_type(&name)
                .unwrap_or_else(|| Type::Own(name, vec![Type::Unknown; args.len()])),
        )
    }
}

/// A case's test: its condition and what its object is bound to.
pub(crate) struct CaseTest {
    pub test: String,
    pub bound: String,
}

impl Gen<'_> {
    /// `v` a case carrying data (`Mode.Charging`, a generic enum's tested
    /// with `on`'s arguments): bound to a temporary; its fields' names.
    fn case_test(&mut self, path: &[Name], v: &str, on: Option<&Type>) -> (CaseTest, Vec<String>) {
        let (of, case) = self.case_path(path);
        let c = self.temp();
        let given = match on.map(Type::non_null) {
            Some(Type::Own(_, args)) if !args.is_empty() => {
                let java: Vec<String> = args.iter().map(|a| self.java(a, true)).collect();
                format!("<{}>", java.join(", "))
            }
            _ => String::new(),
        };
        let test = format!(
            "{v} instanceof {}{given} {c}",
            self.qualified(of.as_deref(), &case)
        );
        let fields = self
            .index
            .case_fields(of.as_deref(), &case)
            .iter()
            .map(|f| Self::ident(f))
            .collect();
        (CaseTest { test, bound: c }, fields)
    }

    /// `Mode.Idle` (a case without data: its one object) or `Monster m`
    /// (a type, `m` bound) as a condition.
    fn type_test(
        &mut self,
        ty: &yog_script_analyze::ast::Type,
        bind: Option<&str>,
        v: &str,
    ) -> Option<String> {
        let TypeKind::Path { path, .. } = &ty.kind else {
            return None;
        };
        let of = path.len().checked_sub(2).map(|k| path[k].text.clone());
        let case = path.last().map(|n| n.text.as_str()).unwrap_or_default();
        let enum_case = of
            .as_deref()
            .is_some_and(|o| self.index.kind(&self.index.named(o)) == Some(Kind::Enum));
        if enum_case {
            let written = format!(
                "{}.{}",
                self.own(of.as_deref().unwrap_or_default()),
                Self::ident(case)
            );
            if self.index.case_fields(of.as_deref(), case).is_empty() {
                return Some(format!("{v} == {written}"));
            }
            return Some(match bind {
                Some(b) => format!("{v} instanceof {written} {}", Self::ident(b)),
                None => format!("{v} instanceof {written}"),
            });
        }
        let t = self.written(ty, true);
        Some(match bind {
            Some(b) => format!("{v} instanceof {t} {}", Self::ident(b)),
            None => format!("{v} instanceof {t}"),
        })
    }

    /// A property pattern's object: `v` itself, or — a type written
    /// (`Monster { … }`) — tested to be one and bound; the tests, its type,
    /// what reads it.
    fn property_target(
        &mut self,
        ty: Option<&yog_script_analyze::ast::Type>,
        v: &str,
        on: Option<&Type>,
    ) -> (Vec<String>, Option<Type>, String) {
        match ty {
            Some(written) => {
                let bound = self.temp();
                let t = self.written(written, true);
                let of = self.types_of_written(written);
                (vec![format!("{v} instanceof {t} {bound}")], of, bound)
            }
            None => (vec![format!("{v} != null")], on.cloned(), v.to_string()),
        }
    }

    /// A case's enum (its Java name: the runtime's for `Result`) and name.
    fn case_path(&self, path: &[Name]) -> (Option<String>, String) {
        let case = path.last().map(|n| n.text.clone()).unwrap_or_default();
        let of = match path.len().checked_sub(2) {
            Some(k) => Some(path[k].text.clone()),
            None => self.index.enum_of(&case),
        };
        (of, case)
    }

    /// A case's Java type: `Mode.Charging`, the runtime's `Result.Ok`.
    fn qualified(&self, of: Option<&str>, case: &str) -> String {
        match of {
            Some(o) => format!("{}.{case}", self.own(o)),
            None => case.to_string(),
        }
    }
}

/// Whether pattern `p` binds names it needs an `if` for (`let x`).
pub(crate) fn binds(p: &Pattern) -> bool {
    match p {
        Pattern::Let(_) => true,
        Pattern::Variant { args, .. } => args.iter().any(binds),
        Pattern::Tuple(items) => items.iter().any(binds),
        Pattern::Property { fields, .. } => fields.iter().any(|(_, f)| binds(f)),
        Pattern::Or(a, b) | Pattern::And(a, b) => binds(a) || binds(b),
        Pattern::Not(inner) => binds(inner),
        _ => false,
    }
}

/// Item `i`'s type of tuple type `on`.
fn tuple_item(on: Option<&Type>, i: usize) -> Option<Type> {
    match on.map(Type::non_null) {
        Some(Type::Tuple(items, _)) => items.get(i).cloned(),
        _ => None,
    }
}
