//! Enums, one model for all, as `System.Enum`: a class whose values are
//! numbers — its cases' (`Wood = 1`, else one more than the last), and any
//! made of one (`(Tier)5`, `A | B`), one object each — a case carrying
//! data (`Charging(i32 Rate)`) a final subclass of its number and fields.
//! A case without data is matched by `==`, one with by `instanceof`.

use std::collections::HashMap;

use yog_script_analyze::ast::{
    BinOp, Decl, Expr, ExprKind, Generic, Type as Written, UnOp, Variant,
};
use yog_script_analyze::{Type, builtin_type};
use yog_script_ids::jvm::enums::{
    CARRYING, COMPARE, FLAGS, OF, TABLE, TABLE_CLASS, TABLE_OF, VALUE, WIDTH,
};
use yog_script_ids::jvm::{EQUALS, HASH, OBJECTS};
use yog_script_ids::members::TO_STRING;
use yog_script_ids::system::{ENUM, FLAGS as FLAGS_ATTRIBUTE};

use crate::Gen;
use crate::decls::{Place, generic_list};
use crate::index::Kind;
use crate::modifier;
use yog_script_ids::java::{
    BOOLEAN, CLASS, CLASS_LITERAL, EXTENDS, FINAL, INSTANCEOF, INT, LONG, LONG_SUFFIX, NEW,
    OVERRIDE, PERMITS, PRIVATE, PROTECTED, PUBLIC, RETURN, SEALED, STATIC, SUPER, THIS,
};
use yog_script_ids::jvm::OBJECT;

impl Gen<'_> {
    /// Enum `d` (`name` of `generics`, numbers of `base`).
    pub(crate) fn enum_decl(
        &mut self,
        d: &Decl,
        (name, generics, base): (&str, &[Generic], Option<&Written>),
        variants: &[Variant],
        body: &[Decl],
    ) {
        let Some(numbers) = self.case_numbers(variants) else {
            return;
        };
        let g = generic_list(generics);
        let base_class = self.index.place(ENUM, 0);
        let stat = if self.nested {
            modifier(STATIC)
        } else {
            String::new()
        };
        let data: Vec<String> = variants
            .iter()
            .filter(|v| !v.fields.is_empty())
            .map(|v| format!("{name}.{}", Self::ident(&v.name.text)))
            .collect();
        let head = if data.is_empty() {
            format!("{PUBLIC} {stat}{FINAL} {CLASS} {name}{g} {EXTENDS} {base_class}")
        } else {
            format!(
                "{PUBLIC} {stat}{SEALED} {CLASS} {name}{g} {EXTENDS} {base_class} {PERMITS} {}",
                data.join(", ")
            )
        };
        self.code.open(head);
        self.describe(d);
        self.code
            .line(self.table(d, name, base, (variants, &numbers)));
        for (v, n) in variants.iter().zip(&numbers) {
            if v.fields.is_empty() {
                let case = Self::ident(&v.name.text);
                self.code.line(format!(
                    "{PUBLIC} {STATIC} {FINAL} {name} {case} = {TABLE}.{OF}({n}{LONG_SUFFIX});"
                ));
            }
        }
        self.code.line(format!(
            "{PROTECTED} {name}({LONG} value) {{ {SUPER}(value); }}"
        ));
        self.code.line(format!(
            "{OVERRIDE} {PROTECTED} {base_class}.{TABLE_CLASS}<?> {TABLE_OF}() {{ {RETURN} {TABLE}; }}"
        ));
        self.code.line(format!(
            "{PUBLIC} {STATIC} {name} {OF}({LONG} value) {{ {RETURN} {TABLE}.{OF}(value); }}"
        ));
        for (v, n) in variants.iter().zip(&numbers) {
            if !v.fields.is_empty() {
                self.data_case((name, &g, generics), v, *n);
            }
        }
        self.members(body, Place::Class);
        self.code.close("");
    }

    /// `TABLE`: its cases' names and numbers, its base type's width, which
    /// carry data, whether it's `[Flags]`.
    fn table(
        &self,
        d: &Decl,
        name: &str,
        base: Option<&Written>,
        (variants, numbers): (&[Variant], &[i64]),
    ) -> String {
        let base_class = self.index.place(ENUM, 0);
        let names: Vec<String> = variants
            .iter()
            .map(|v| self.code.str(&v.name.text))
            .collect();
        let values: Vec<String> = numbers
            .iter()
            .map(|n| format!("{n}{LONG_SUFFIX}"))
            .collect();
        let underlying = base
            .and_then(|b| match &b.kind {
                yog_script_analyze::ast::TypeKind::Path { path, .. } => {
                    path.last().and_then(|n| builtin_type(&n.text))
                }
                _ => None,
            })
            .unwrap_or(Type::I32);
        let (bits, signed) = underlying.integer_width().unwrap_or((i32::BITS, true));
        let carrying: Vec<String> = variants
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.fields.is_empty())
            .map(|(i, _)| i.to_string())
            .collect();
        let mut out = format!(
            "{PRIVATE} {STATIC} {FINAL} {base_class}.{TABLE_CLASS}<{name}> {TABLE} = {NEW} {base_class}.{TABLE_CLASS}<>({name}{CLASS_LITERAL}, {name}::{NEW}, {NEW} String[] {{ {} }}, {NEW} {LONG}[] {{ {} }}).{WIDTH}({bits}, {signed})",
            names.join(", "),
            values.join(", ")
        );
        if !carrying.is_empty() {
            out.push_str(&format!(".{CARRYING}({})", carrying.join(", ")));
        }
        if crate::library::marked(d, FLAGS_ATTRIBUTE) {
            out.push_str(&format!(".{FLAGS}()"));
        }
        out.push(';');
        out
    }

    /// A case carrying data: a final subclass of its number, its fields
    /// read as a record's (`Rate()`), equal by them.
    fn data_case(
        &mut self,
        (name, g, generics): (&str, &str, &[Generic]),
        v: &Variant,
        number: i64,
    ) {
        let case = Self::ident(&v.name.text);
        self.code.open(format!(
            "{PUBLIC} {STATIC} {FINAL} {CLASS} {case}{g} {EXTENDS} {name}{g}"
        ));
        let fields: Vec<(String, String)> = v
            .fields
            .iter()
            .map(|f| (self.written(&f.ty, false), Self::ident(&f.name.text)))
            .collect();
        for (t, f) in &fields {
            self.code.line(format!("{PUBLIC} {FINAL} {t} {f};"));
        }
        let params: Vec<String> = fields.iter().map(|(t, f)| format!("{t} {f}")).collect();
        self.code
            .open(format!("{PUBLIC} {case}({})", params.join(", ")));
        self.code.line(format!("{SUPER}({number}{LONG_SUFFIX});"));
        for (_, f) in &fields {
            self.code.line(format!("{THIS}.{f} = {f};"));
        }
        self.code.close("");
        for (t, f) in &fields {
            self.code
                .line(format!("{PUBLIC} {t} {f}() {{ {RETURN} {f}; }}"));
        }
        let arity = generics.iter().filter(|g| !g.lifetime).count();
        let tested = if arity == 0 {
            case.clone()
        } else {
            format!("{case}<{}>", vec!["?"; arity].join(", "))
        };
        let same: Vec<String> = fields
            .iter()
            .map(|(_, f)| format!(" && {OBJECTS}.{EQUALS}({f}, that.{f})"))
            .collect();
        self.code.line(format!(
            "{OVERRIDE} {PUBLIC} {BOOLEAN} equals({OBJECT} other) {{ {RETURN} other {INSTANCEOF} {tested} that{}; }}",
            same.concat()
        ));
        let hashed: Vec<&str> = fields.iter().map(|(_, f)| f.as_str()).collect();
        self.code.line(format!(
            "{OVERRIDE} {PUBLIC} {INT} hashCode() {{ {RETURN} {OBJECTS}.{HASH}({VALUE}(), {}); }}",
            hashed.join(", ")
        ));
        // Each field as text, as `object`'s `ToString` writes it.
        let shown_by = self.index.host(&Type::Object).unwrap_or_default();
        let shown: Vec<String> = fields
            .iter()
            .map(|(_, f)| format!("{shown_by}.{TO_STRING}({f})"))
            .collect();
        self.code.line(format!(
            "{OVERRIDE} {PUBLIC} String toString() {{ {RETURN} \"{}(\" + {} + \")\"; }}",
            v.name.text,
            shown.join(" + \", \" + ")
        ));
        self.code.close("");
    }

    /// Each case's number: its own (a constant of numbers and cases
    /// before it), else one more than the last's, from 0.
    fn case_numbers(&mut self, variants: &[Variant]) -> Option<Vec<i64>> {
        let mut known: HashMap<String, i128> = HashMap::new();
        let mut out = Vec::new();
        let mut next: i128 = 0;
        for v in variants {
            let n = match &v.value {
                Some(e) => match number(e, &known) {
                    Some(n) => n,
                    None => {
                        self.not_yet(
                            "this case's number (a constant of numbers and cases)",
                            e.span,
                        );
                        return None;
                    }
                },
                None => next,
            };
            known.insert(v.name.text.clone(), n);
            next = n + 1;
            out.push(n as i64);
        }
        Some(out)
    }
}

impl Gen<'_> {
    /// `e`'s enum's class, when `e` is of one.
    fn enum_class(&self, e: &Expr) -> Option<String> {
        match self.types.of(e).map(Type::non_null) {
            Some(Type::Own(name, args))
                if self.index.kind(&(name.clone(), args.len())) == Some(Kind::Enum) =>
            {
                Some(self.index.place(name, args.len()))
            }
            _ => None,
        }
    }

    /// `a | b` of enums: their numbers', an enum; `a < b`: by number.
    pub(crate) fn enum_binary(&mut self, op: BinOp, left: &Expr, right: &Expr) -> Option<String> {
        let class = self.enum_class(left)?;
        let bits = match op {
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => Some(crate::exprs::op_text(op)),
            _ => None,
        };
        let order = matches!(op, BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq);
        if bits.is_none() && !order {
            return None;
        }
        let (l, r) = (self.expr(left), self.expr(right));
        Some(match bits {
            Some(b) => format!("{class}.{OF}({l}.{VALUE}() {b} {r}.{VALUE}())"),
            None => format!(
                "({}.{COMPARE}({l}, {r}) {} 0)",
                self.index.place(ENUM, 0),
                crate::exprs::op_text(op)
            ),
        })
    }

    /// `~a` of an enum: its number's bits flipped, an enum.
    pub(crate) fn enum_unary(&mut self, op: UnOp, e: &Expr) -> Option<String> {
        if op != UnOp::BitNot {
            return None;
        }
        let class = self.enum_class(e)?;
        let v = self.expr(e);
        Some(format!("{class}.{OF}(~{v}.{VALUE}())"))
    }

    /// `(i32)a` of an enum: its number; `(Tier)n`: the enum of number `n`.
    pub(crate) fn enum_cast(&mut self, cast: &Expr, inner: &Expr) -> Option<String> {
        if let Some(_class) = self.enum_class(inner) {
            let to = self.java(self.types.of(cast)?, false);
            let v = self.expr(inner);
            return Some(format!("(({to}) {v}.{VALUE}())"));
        }
        let class = self.enum_class(cast)?;
        let v = self.expr(inner);
        Some(format!("{class}.{OF}(({LONG}) {v})"))
    }
}

/// Constant `e`'s number: numbers, cases named before it, and what
/// combines them.
fn number(e: &Expr, known: &HashMap<String, i128>) -> Option<i128> {
    match &e.kind {
        ExprKind::Int(v) => Some(i128::from(*v)),
        ExprKind::Ident(n) => known.get(n).copied(),
        ExprKind::Member { name, .. } => known.get(&name.text).copied(),
        ExprKind::Cast { expr, .. } => number(expr, known),
        ExprKind::Unary {
            op: UnOp::Neg,
            expr,
        } => number(expr, known).map(|n| -n),
        ExprKind::Unary {
            op: UnOp::BitNot,
            expr,
        } => number(expr, known).map(|n| !n),
        ExprKind::Binary { op, left, right } => {
            let (a, b) = (number(left, known)?, number(right, known)?);
            Some(match op {
                BinOp::BitOr => a | b,
                BinOp::BitAnd => a & b,
                BinOp::BitXor => a ^ b,
                BinOp::Shl => a.checked_shl(u32::try_from(b).ok()?)?,
                BinOp::Shr => a.checked_shr(u32::try_from(b).ok()?)?,
                BinOp::Add => a.checked_add(b)?,
                BinOp::Sub => a.checked_sub(b)?,
                BinOp::Mul => a.checked_mul(b)?,
                _ => return None,
            })
        }
        _ => None,
    }
}
