//! One `{x:Bind expression, Mode=…, BindBack=…, Fallback=…}`.

use yog_ids::yui_vm as v;

use super::expr::{Ex, Scope, parse_expr};
use crate::yui::{Element, err};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Mode {
    OneWay,
    TwoWay,
}

/// What a binding reads: an expression of the view model, or Rust made
/// already (a `.yres` text, looked up on every update).
#[derive(Debug, Clone)]
pub(super) enum Bound {
    Expr(Ex),
    Rust(String),
}

/// One `{x:Bind ...}` (or a `@Set.name` text, compiled as a binding).
#[derive(Debug, Clone)]
pub(super) struct Binding {
    /// The attribute value as written, for comments.
    pub(super) raw: String,
    pub(super) bound: Bound,
    pub(super) mode: Mode,
    pub(super) bind_back: Option<String>,
    pub(super) fallback: Option<String>,
}

impl Binding {
    /// A one-way binding of `bound`, written `raw`.
    pub(super) fn one_way(raw: &str, bound: Bound) -> Binding {
        Binding {
            raw: raw.to_string(),
            bound,
            mode: Mode::OneWay,
            bind_back: None,
            fallback: None,
        }
    }

    /// Its expression, when it's one.
    pub(super) fn expr(&self) -> Option<&Ex> {
        match &self.bound {
            Bound::Expr(e) => Some(e),
            Bound::Rust(_) => None,
        }
    }

    /// Whether it reads through a `?.`: an `Option` to fall back from.
    pub(super) fn has_opt(&self) -> bool {
        self.expr().is_some_and(Ex::has_opt)
    }

    /// The bound value as Rust in `scope`.
    pub(super) fn rust(&self, scope: Scope, el: &Element) -> Result<String, String> {
        match &self.bound {
            Bound::Rust(r) => Ok(r.clone()),
            Bound::Expr(e) => e
                .emit(scope, false)
                .map_err(|e| format!("line {}: {e}", el.line)),
        }
    }

    /// Its expression as Rust in `scope`; an error at `el` when it has none.
    pub(super) fn emit(&self, scope: Scope, el: &Element) -> Result<String, String> {
        match self.expr() {
            Some(e) => e
                .emit(scope, false)
                .map_err(|e| format!("line {}: {e}", el.line)),
            None => err(el, format_args!("`{}` needs {{x:Bind ...}}", self.raw)),
        }
    }

    /// What writes a new value back to the view model: its `BindBack`
    /// method's call on `arg`, or (two-way) its field set to `value`.
    pub(super) fn write_back(&self, arg: &str, value: &str) -> Result<WriteBack, String> {
        Ok(match (&self.bind_back, self.mode, self.expr()) {
            (Some(m), _, _) => WriteBack::Stmt(format!("vm.{m}({arg});")),
            (None, Mode::TwoWay, Some(e)) if e.is_place() => {
                WriteBack::Stmt(format!("{} = {value};", e.emit(Scope::Screen, false)?))
            }
            (None, Mode::TwoWay, _) => WriteBack::Nowhere,
            _ => WriteBack::OneWay,
        })
    }
}

/// How a binding writes back: not at all, by this statement, or (two-way
/// without a field or method) nowhere — a mistake.
pub(super) enum WriteBack {
    OneWay,
    Stmt(String),
    Nowhere,
}

/// Splits on commas outside parentheses and quotes.
fn split_top(s: &str) -> Vec<&str> {
    let (mut depth, mut quote, mut start) = (0i32, false, 0);
    let mut out = Vec::new();
    for (i, c) in s.char_indices() {
        match c {
            '\'' => quote = !quote,
            '(' if !quote => depth += 1,
            ')' if !quote => depth -= 1,
            ',' if !quote && depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// What joins a binding option's name and value: `Mode=TwoWay`.
const OPTION_IS: char = '=';

/// `value` as a binding; none when it isn't one.
pub(super) fn parse_binding(value: &str) -> Option<Result<Binding, String>> {
    let inner = value
        .trim()
        .strip_prefix(yog_ids::yui_markup::BIND)?
        .strip_suffix(yog_ids::yui_markup::BIND_CLOSE)?;
    Some(read_binding(value, inner))
}

fn read_binding(value: &str, inner: &str) -> Result<Binding, String> {
    let mut parts = split_top(inner).into_iter().map(str::trim);
    let expr = parts
        .next()
        .filter(|p| !p.is_empty())
        .ok_or("empty binding — write {x:Bind expression}")?;
    let mut b = Binding::one_way(
        value,
        Bound::Expr(parse_expr(expr).map_err(|e| format!("{value}: {e}"))?),
    );
    for part in parts {
        let Some((k, o)) = part.split_once(OPTION_IS) else {
            return Err(format!(
                "{value}: expected Mode=, BindBack= or Fallback=, got `{part}`"
            ));
        };
        let o = o.trim();
        match k.trim() {
            v::MODE => {
                b.mode = match o {
                    v::ONE_WAY | v::ONE_TIME => Mode::OneWay,
                    v::TWO_WAY => Mode::TwoWay,
                    _ => return Err(format!("{value}: Mode is OneWay, OneTime or TwoWay")),
                }
            }
            v::BIND_BACK => b.bind_back = Some(o.to_string()),
            v::FALLBACK => b.fallback = Some(o.trim_matches('\'').to_string()),
            other => return Err(format!("{value}: unknown binding option `{other}`")),
        }
    }
    Ok(b)
}

/// Compiles an attribute value `{x:Bind expr}` into a Rust expression that
/// reads the view model `vm` (bare names become `vm.name`, `Type::f(x)`
/// gets `&vm.x`, `a?.b` stays a `?`-chain — wrap it in a closure returning
/// `Option`). `None` when `value` isn't a binding. Options (`Mode=`,
/// `BindBack=`, `Fallback=`) are parsed and ignored here. Shared with the
/// `.yui` component generator so there's one binding syntax.
pub(crate) fn compile_bind(value: &str) -> Option<Result<String, String>> {
    Some(parse_binding(value)?.and_then(|b| match b.bound {
        Bound::Expr(e) => e.emit(Scope::Screen, false),
        Bound::Rust(r) => Ok(r),
    }))
}
