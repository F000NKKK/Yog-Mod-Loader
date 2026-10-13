//! View-model screens: a `.yui` whose `<Screen>` has `x:ViewModel="MyVm"`
//! compiles into a `yog_kit::View` — the static widget tree (built by
//! [`crate::yui`]'s generator), compiled `{x:Bind ...}` bindings patching
//! it, commands, two-way text boxes and toggles, lists with item templates,
//! and the `register`/`open`/`close` functions. See `docs/mvvm.md` in
//! Yog-IDLE for the format.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::yui::{self, Element, RAW_CALL, RAW_VALUE, UI_USE, err};

/// The codegen version checked by `yog_kit::__codegen::require`.
const CODEGEN: u32 = 1;

const STATES: &[(&str, &str)] = &[
    ("hover", "HOVER"),
    ("pressed", "PRESSED"),
    ("focused", "FOCUSED"),
    ("selected", "SELECTED"),
    ("checked", "CHECKED"),
    ("disabled", "DISABLED"),
];

// ── binding expressions ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Num(String),
    Str(String),
    PathSep,
    Dot,
    OptDot,
    Open,
    Close,
    Comma,
    Eq,
    Ne,
    Not,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let next = cs.get(i + 1).copied();
        match c {
            _ if c.is_whitespace() => i += 1,
            _ if c.is_alphabetic() || c == '_' => {
                let s = i;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(cs[s..i].iter().collect()));
            }
            _ if c.is_ascii_digit() || (c == '-' && next.is_some_and(|n| n.is_ascii_digit())) => {
                let s = i;
                i += 1;
                while i < cs.len()
                    && (cs[i].is_ascii_alphanumeric()
                        || cs[i] == '_'
                        || (cs[i] == '.' && cs.get(i + 1).is_some_and(|n| n.is_ascii_digit())))
                {
                    i += 1;
                }
                out.push(Tok::Num(cs[s..i].iter().collect()));
            }
            '\'' => {
                let s = i + 1;
                i += 1;
                while i < cs.len() && cs[i] != '\'' {
                    i += 1;
                }
                if i >= cs.len() {
                    return Err("unterminated 'string'".into());
                }
                out.push(Tok::Str(cs[s..i].iter().collect()));
                i += 1;
            }
            ':' if next == Some(':') => {
                out.push(Tok::PathSep);
                i += 2;
            }
            '?' if next == Some('.') => {
                out.push(Tok::OptDot);
                i += 2;
            }
            '=' if next == Some('=') => {
                out.push(Tok::Eq);
                i += 2;
            }
            '!' if next == Some('=') => {
                out.push(Tok::Ne);
                i += 2;
            }
            '!' => {
                out.push(Tok::Not);
                i += 1;
            }
            '.' => {
                out.push(Tok::Dot);
                i += 1;
            }
            '(' => {
                out.push(Tok::Open);
                i += 1;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            _ => {
                return Err(format!(
                    "unexpected `{c}` (bindings allow paths, calls, literals, !, == and !=; use a view-model method for anything else)"
                ));
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct Seg {
    opt: bool,
    name: String,
    args: Option<Vec<Ex>>,
}

#[derive(Debug, Clone)]
enum Head {
    Ident(String, Option<Vec<Ex>>),
    Type(String, Option<Vec<Ex>>),
    Lit(String),
    Paren(Box<Ex>),
}

#[derive(Debug, Clone)]
enum Ex {
    Post(Head, Vec<Seg>),
    Not(Box<Ex>),
    Cmp(Box<Ex>, &'static str, Box<Ex>),
}

struct ExParser {
    toks: Vec<Tok>,
    pos: usize,
}

impl ExParser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<Ex, String> {
        let a = self.unary()?;
        let op = if self.eat(&Tok::Eq) {
            "=="
        } else if self.eat(&Tok::Ne) {
            "!="
        } else {
            return Ok(a);
        };
        let b = self.unary()?;
        Ok(Ex::Cmp(Box::new(a), op, Box::new(b)))
    }
    fn unary(&mut self) -> Result<Ex, String> {
        if self.eat(&Tok::Not) {
            return Ok(Ex::Not(Box::new(self.postfix()?)));
        }
        self.postfix()
    }
    fn call(&mut self) -> Result<Option<Vec<Ex>>, String> {
        if !self.eat(&Tok::Open) {
            return Ok(None);
        }
        let mut args = Vec::new();
        if self.eat(&Tok::Close) {
            return Ok(Some(args));
        }
        loop {
            args.push(self.expr()?);
            if self.eat(&Tok::Close) {
                return Ok(Some(args));
            }
            if !self.eat(&Tok::Comma) {
                return Err("expected `,` or `)`".into());
            }
        }
    }
    fn postfix(&mut self) -> Result<Ex, String> {
        let head = match self.bump() {
            Some(Tok::Ident(id)) => {
                if self.peek() == Some(&Tok::PathSep) {
                    let mut path = id;
                    while self.eat(&Tok::PathSep) {
                        match self.bump() {
                            Some(Tok::Ident(n)) => {
                                path.push_str("::");
                                path.push_str(&n);
                            }
                            _ => return Err("expected a name after `::`".into()),
                        }
                    }
                    Head::Type(path, self.call()?)
                } else if id == "true" || id == "false" {
                    Head::Lit(id)
                } else {
                    Head::Ident(id.clone(), self.call()?)
                }
            }
            Some(Tok::Num(n)) => Head::Lit(n),
            Some(Tok::Str(s)) => Head::Lit(format!("{s:?}")),
            Some(Tok::Open) => {
                let e = self.expr()?;
                if !self.eat(&Tok::Close) {
                    return Err("expected `)`".into());
                }
                Head::Paren(Box::new(e))
            }
            Some(t) => return Err(format!("unexpected {t:?}")),
            None => return Err("expected an expression".into()),
        };
        let mut segs = Vec::new();
        loop {
            let opt = if self.eat(&Tok::Dot) {
                false
            } else if self.eat(&Tok::OptDot) {
                true
            } else {
                break;
            };
            let Some(Tok::Ident(name)) = self.bump() else {
                return Err("expected a name after `.`".into());
            };
            let args = self.call()?;
            segs.push(Seg { opt, name, args });
        }
        Ok(Ex::Post(head, segs))
    }
}

fn parse_expr(src: &str) -> Result<Ex, String> {
    let mut p = ExParser {
        toks: lex(src)?,
        pos: 0,
    };
    let e = p.expr()?;
    if p.pos < p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.pos]));
    }
    Ok(e)
}

/// Which names a binding sees.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Screen,
    Template,
}

impl Ex {
    fn has_opt(&self) -> bool {
        match self {
            Ex::Post(h, segs) => {
                segs.iter()
                    .any(|s| s.opt || s.args.iter().flatten().any(Ex::has_opt))
                    || match h {
                        Head::Ident(_, a) | Head::Type(_, a) => a.iter().flatten().any(Ex::has_opt),
                        Head::Paren(e) => e.has_opt(),
                        Head::Lit(_) => false,
                    }
            }
            Ex::Not(e) => e.has_opt(),
            Ex::Cmp(a, _, b) => a.has_opt() || b.has_opt(),
        }
    }

    /// A field path (`search`, `vm.state.name`, `item.id`): assignable, and
    /// passed by reference to type-path calls.
    fn is_place(&self) -> bool {
        match self {
            Ex::Post(Head::Ident(n, None), segs) => {
                n != "cx" && segs.iter().all(|s| !s.opt && s.args.is_none())
            }
            _ => false,
        }
    }

    fn emit(&self, scope: Scope, command: bool) -> Result<String, String> {
        let args = |a: &Option<Vec<Ex>>, by_ref: bool| -> Result<String, String> {
            let Some(a) = a else {
                return Ok(String::new());
            };
            let parts = a
                .iter()
                .map(|e| {
                    let s = e.emit(scope, command)?;
                    Ok(if by_ref && e.is_place() {
                        format!("&{s}")
                    } else {
                        s
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(format!("({})", parts.join(", ")))
        };
        Ok(match self {
            Ex::Post(h, segs) => {
                let mut s = match h {
                    Head::Lit(l) => l.clone(),
                    Head::Paren(e) => format!("({})", e.emit(scope, command)?),
                    Head::Type(p, a) => format!("{p}{}", args(a, true)?),
                    Head::Ident(n, a) => {
                        let root = match (n.as_str(), scope) {
                            ("vm", _) => "vm".to_string(),
                            ("cx", _) if command => "cx".to_string(),
                            ("cx", _) => return Err("`cx` is only available in commands".into()),
                            ("item" | "index", Scope::Template) => n.clone(),
                            ("item" | "index", Scope::Screen) => {
                                return Err(format!("`{n}` is only available in item templates"));
                            }
                            (_, Scope::Screen) => format!("vm.{n}"),
                            (_, Scope::Template) => format!("item.{n}"),
                        };
                        format!("{root}{}", args(a, false)?)
                    }
                };
                for seg in segs {
                    if seg.opt {
                        s.push('?');
                    }
                    let _ = write!(s, ".{}{}", seg.name, args(&seg.args, false)?);
                }
                s
            }
            Ex::Not(e) => format!("!{}", e.emit(scope, command)?),
            Ex::Cmp(a, op, b) => format!(
                "{} {op} {}",
                a.emit(scope, command)?,
                b.emit(scope, command)?
            ),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    OneWay,
    TwoWay,
}

/// One `{x:Bind ...}` (or a `@Set.name` text, compiled as a binding).
#[derive(Debug, Clone)]
struct Binding {
    /// The attribute value as written, for comments.
    raw: String,
    expr: Option<Ex>,
    /// Pre-emitted Rust (resource texts).
    rust: Option<String>,
    mode: Mode,
    bind_back: Option<String>,
    fallback: Option<String>,
}

impl Binding {
    /// The bound value as Rust in `scope`.
    fn rust(&self, scope: Scope, el: &Element) -> Result<String, String> {
        match (&self.rust, &self.expr) {
            (Some(r), _) => Ok(r.clone()),
            (None, Some(e)) => e
                .emit(scope, false)
                .map_err(|e| format!("line {}: {e}", el.line)),
            (None, None) => unreachable!("a binding has an expression"),
        }
    }
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

fn parse_binding(value: &str) -> Option<Result<Binding, String>> {
    let inner = value.trim().strip_prefix("{x:Bind")?.strip_suffix('}')?;
    Some((|| {
        let mut b = Binding {
            raw: value.to_string(),
            expr: None,
            rust: None,
            mode: Mode::OneWay,
            bind_back: None,
            fallback: None,
        };
        for (i, part) in split_top(inner).into_iter().enumerate() {
            let part = part.trim();
            if i == 0 {
                if part.is_empty() {
                    return Err("empty binding — write {x:Bind expression}".to_string());
                }
                b.expr = Some(parse_expr(part).map_err(|e| format!("{value}: {e}"))?);
                continue;
            }
            let Some((k, v)) = part.split_once('=') else {
                return Err(format!(
                    "{value}: expected Mode=, BindBack= or Fallback=, got `{part}`"
                ));
            };
            let v = v.trim();
            match k.trim() {
                "Mode" => {
                    b.mode = match v {
                        "OneWay" | "OneTime" => Mode::OneWay,
                        "TwoWay" => Mode::TwoWay,
                        _ => return Err(format!("{value}: Mode is OneWay, OneTime or TwoWay")),
                    }
                }
                "BindBack" => b.bind_back = Some(v.to_string()),
                "Fallback" => b.fallback = Some(v.trim_matches('\'').to_string()),
                other => return Err(format!("{value}: unknown binding option `{other}`")),
            }
        }
        Ok(b)
    })())
}

/// Compiles an attribute value `{x:Bind expr}` into a Rust expression that
/// reads the view model `vm` (bare names become `vm.name`, `Type::f(x)`
/// gets `&vm.x`, `a?.b` stays a `?`-chain — wrap it in a closure returning
/// `Option`). `None` when `value` isn't a binding. Options (`Mode=`,
/// `BindBack=`, `Fallback=`) are parsed and ignored here. Shared with the
/// `.yui` component generator so there's one binding syntax.
pub(crate) fn compile_bind(value: &str) -> Option<Result<String, String>> {
    let b = match parse_binding(value)? {
        Ok(b) => b,
        Err(e) => return Some(Err(e)),
    };
    Some(b.expr.expect("parsed").emit(Scope::Screen, false))
}

// ── generator ────────────────────────────────────────────────────────────────

/// What a bound attribute converts to and how it's applied.
#[derive(Clone, Copy, PartialEq)]
enum Target {
    Text,
    Placeholder,
    Item,
    Source,
    Count,
    Color(&'static str),
    Enabled,
    Visible,
    Checked,
    Size(&'static str),
    Number(&'static str),
    ScrollKey,
}

fn target(kind: &str, attr: &str) -> Option<Target> {
    Some(match attr {
        "text" if matches!(kind, "Label" | "Button" | "TextBox") => Target::Text,
        "placeholder" if kind == "TextBox" => Target::Placeholder,
        "item" if matches!(kind, "ItemSlot" | "InvSlot") => Target::Item,
        "source" if matches!(kind, "Image" | "Surface") => Target::Source,
        "count" if kind == "InvSlot" => Target::Count,
        "bg" => Target::Color("bg"),
        "color" => Target::Color("color"),
        "focus_color" => Target::Color("focus_color"),
        "enabled" => Target::Enabled,
        "visible" => Target::Visible,
        "checked" if kind == "Button" => Target::Checked,
        "w" => Target::Size("w"),
        "h" => Target::Size("h"),
        "gap" => Target::Number("gap"),
        "font_scale" => Target::Number("font_scale"),
        "flex" => Target::Number("flex"),
        "min_w" => Target::Number("min_w"),
        "min_h" => Target::Number("min_h"),
        "scroll_key" if kind == "ScrollView" => Target::ScrollKey,
        _ => return None,
    })
}

/// Where generated update code writes: the view or a template instance.
struct Ctx {
    scope: Scope,
    /// `self.` or `inst.`
    fields: &'static str,
    /// The tree variable: `root` or `w`.
    tree: &'static str,
    /// List owner of actions (0 = screen).
    owner: u16,
    /// Slot fields declared (name, type) — for templates.
    slots: Vec<(String, String)>,
    update: String,
}

impl Ctx {
    fn new(scope: Scope, fields: &'static str, tree: &'static str, owner: u16) -> Self {
        Ctx {
            scope,
            fields,
            tree,
            owner,
            slots: Vec::new(),
            update: String::new(),
        }
    }
}

struct VmGen<'a> {
    file: String,
    mod_id: Option<&'a str>,
    resources: HashMap<String, String>,
    styles: HashMap<String, Vec<(String, String)>>,
    ids: yui::Names,
    /// View fields (name, type, init).
    view_fields: Vec<(String, String, String)>,
    parts: String,
    dispatch: Vec<(String, String)>,
    templates: String,
    n_slots: usize,
    n_lists: u16,
    n_templates: usize,
    n_scrollers: u16,
    n_text_boxes: u16,
    /// Next command slot per owner.
    cmd_slots: HashMap<u16, u16>,
}

fn path_lit(path: &[u16]) -> String {
    format!(
        "&[{}]",
        path.iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

impl VmGen<'_> {
    fn slot(&mut self, ctx: &mut Ctx, ty: &str) -> String {
        let name = format!("b{}", self.n_slots);
        self.n_slots += 1;
        if ctx.scope == Scope::Template && ctx.fields == "inst." {
            ctx.slots.push((name.clone(), ty.to_string()));
        } else {
            self.view_fields.push((
                name.clone(),
                format!("::yog_kit::bind::Slot<{ty}>"),
                "::yog_kit::bind::Slot::new()".into(),
            ));
        }
        name
    }

    fn next_cmd(&mut self, owner: u16) -> u16 {
        let n = self.cmd_slots.entry(owner).or_insert(0);
        let s = *n;
        *n += 1;
        s
    }

    /// Resolves `@local` resources, `x:Name` and `style` into plain attributes.
    fn resolve(&self, el: &Element) -> Result<Element, String> {
        let mut out = el.clone();
        out.attrs.clear();
        let mut style: Vec<(String, String)> = Vec::new();
        for (k, v) in &el.attrs {
            let v = match v.strip_prefix('@') {
                Some(name) if !name.starts_with('@') && !name.contains('.') && !name.is_empty() => {
                    match self.resources.get(name) {
                        Some(r) => r.clone(),
                        None if k == "style" => v.clone(),
                        None => {
                            return err(
                                el,
                                format!("{k}=\"{v}\": no resource `{name}` in <Screen.Resources>"),
                            );
                        }
                    }
                }
                _ => v.clone(),
            };
            match k.as_str() {
                "x:Name" => out.attrs.push(("id".into(), v)),
                "style" => {
                    let name = v.trim().trim_start_matches('@');
                    let Some(s) = self.styles.get(name) else {
                        return err(
                            el,
                            format!(
                                "style=\"{v}\": no <Style x:Key=\"{name}\"> in <Screen.Resources>"
                            ),
                        );
                    };
                    style = s.clone();
                }
                _ => out.attrs.push((k.clone(), v)),
            }
        }
        for (k, v) in style {
            if !out.attrs.iter().any(|(ek, _)| *ek == k) {
                out.attrs.push((k, v));
            }
        }
        Ok(out)
    }

    fn collect_resources(&mut self, res: &Element) -> Result<(), String> {
        let mut pending: Vec<&Element> = Vec::new();
        for r in &res.children {
            let Some(key) = r.attr("x:Key") else {
                return err(r, "needs an x:Key");
            };
            if r.name == "Style" {
                pending.push(r);
                continue;
            }
            let Some(v) = r.attr("value") else {
                return err(
                    r,
                    "needs a value (e.g. <Color x:Key=\"Accent\" value=\"#FFD700\"/>)",
                );
            };
            self.resources.insert(key.to_string(), v.to_string());
        }
        // Styles, resolving based_on chains in any order.
        while !pending.is_empty() {
            let before = pending.len();
            let mut rest = Vec::new();
            for s in pending {
                let key = s.attr("x:Key").unwrap_or_default().to_string();
                let base = s
                    .attr("based_on")
                    .map(|b| b.trim().trim_start_matches('@').to_string());
                let mut attrs: Vec<(String, String)> = match &base {
                    Some(b) => match self.styles.get(b) {
                        Some(a) => a.clone(),
                        None => {
                            rest.push(s);
                            continue;
                        }
                    },
                    None => Vec::new(),
                };
                for (k, v) in &s.attrs {
                    if matches!(k.as_str(), "x:Key" | "target" | "based_on") {
                        continue;
                    }
                    let v = match v.strip_prefix('@') {
                        Some(n) if !n.contains('.') && !n.starts_with('@') => {
                            self.resources.get(n).cloned().ok_or_else(|| {
                                format!("line {}: <Style> {k}=\"{v}\": no resource `{n}`", s.line)
                            })?
                        }
                        _ => v.clone(),
                    };
                    attrs.retain(|(ek, _)| ek != k);
                    attrs.push((k.clone(), v));
                }
                self.styles.insert(key, attrs);
            }
            if rest.len() == before {
                return err(rest[0], "based_on names an unknown (or circular) style");
            }
            pending = rest;
        }
        Ok(())
    }

    /// Converts and applies one bound attribute.
    #[allow(clippy::too_many_arguments)]
    fn bind_attr(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        kind: &str,
        key: &str,
        b: &Binding,
        path: &[u16],
        text_box: Option<u16>,
        scroller: Option<u16>,
    ) -> Result<(), String> {
        let Some(t) = target(kind, key) else {
            return err(el, format!("{key} can't be bound"));
        };
        let expr = b.rust(ctx.scope, el)?;
        let opt = b.expr.as_ref().is_some_and(Ex::has_opt);
        let (ty, conv) = match t {
            Target::Text | Target::Placeholder | Target::Item | Target::Source => {
                ("String", "::yog_kit::bind::text(&(E))".to_string())
            }
            Target::Count => ("u32", "::yog_kit::bind::count(E)".into()),
            Target::Color(_) => ("u32", "::yog_kit::bind::color(E)".into()),
            Target::Enabled | Target::Visible | Target::Checked => {
                ("bool", "::yog_kit::bind::flag(E)".into())
            }
            Target::Size(_) => ("::yog_api::ui::Units", "::yog_kit::bind::units(E)".into()),
            Target::Number(_) => ("f32", "::yog_kit::bind::number(E)".into()),
            Target::ScrollKey => ("u64", "::yog_kit::bind::key(&(E))".into()),
        };
        let value = if opt {
            let fallback = match (&b.fallback, ty) {
                (Some(f), "String") => format!("::std::string::String::from({f:?})"),
                (Some(f), _) => f.clone(),
                (None, _) => "::std::default::Default::default()".into(),
            };
            format!(
                "(|| ::std::option::Option::Some({}))().unwrap_or_else(|| {fallback})",
                conv.replace('E', &expr)
            )
        } else {
            conv.replacen('E', &expr, 1)
        };
        let slot = self.slot(ctx, ty);
        let f = format!("{}{slot}", ctx.fields);
        let w = format!("{}.at_path_mut({})", ctx.tree, path_lit(path));
        let apply = match t {
            Target::Text if kind == "TextBox" => {
                let Some(tb) = text_box else {
                    return err(el, "text boxes in item templates aren't supported");
                };
                format!("u.set_text_box({tb}, {f}.get());")
            }
            Target::Text => {
                format!("{w}.set_text(::std::clone::Clone::clone({f}.get())); u.relayout();")
            }
            Target::Placeholder => {
                format!("::yog_kit::bind::set_placeholder({w}, {f}.get()); u.repaint();")
            }
            Target::Item => format!("::yog_kit::bind::set_item({w}, {f}.get()); u.repaint();"),
            Target::Source => format!("::yog_kit::bind::set_source({w}, {f}.get()); u.relayout();"),
            Target::Count => format!("::yog_kit::bind::set_count({w}, *{f}.get()); u.repaint();"),
            Target::Color(c) => format!("{w}.style.{c} = *{f}.get(); u.repaint();"),
            Target::Enabled => format!("{w}.enabled = *{f}.get(); u.repaint();"),
            Target::Visible => format!("{w}.visible = *{f}.get(); u.relayout();"),
            Target::Checked => {
                format!("::yog_kit::bind::set_checked({w}, *{f}.get()); u.repaint();")
            }
            Target::Size(a) => format!("{w}.style.{a} = *{f}.get(); u.relayout();"),
            Target::Number(a) => format!("{w}.style.{a} = *{f}.get(); u.relayout();"),
            Target::ScrollKey => {
                let Some(s) = scroller else {
                    return err(el, "scroll_key needs a ScrollView outside item templates");
                };
                format!("u.reset_scroll({s});")
            }
        };
        let _ = write!(
            ctx.update,
            "        // @yui {}:{} {key}=\"{}\"\n        if {f}.set({value}) {{ {apply} }}\n",
            self.file,
            el.line,
            b.raw.replace('\n', " ")
        );
        Ok(())
    }

    /// The command statement for `expr`, in screen scope.
    fn command(
        &self,
        el: &Element,
        b: &Binding,
        scope: Scope,
        items: Option<&str>,
    ) -> Result<String, String> {
        let Some(e) = &b.expr else {
            return err(el, "command needs {x:Bind method(...)}");
        };
        let line = el.line;
        match scope {
            Scope::Screen => Ok(format!(
                "{};",
                e.emit(Scope::Screen, true)
                    .map_err(|m| format!("line {line}: {m}"))?
            )),
            Scope::Template => {
                // vm.method(args) / cx.method(): item arguments are cloned
                // out of the item first.
                let Ex::Post(head, segs) = e else {
                    return err(el, "command must be a call: vm.method(...) or cx.close()");
                };
                let rooted = matches!(head, Head::Ident(n, None) if n == "vm" || n == "cx");
                let Some(last) = segs.last().filter(|s| s.args.is_some() && rooted) else {
                    return err(
                        el,
                        "commands in item templates call the view model: write vm.method(...) (or cx....)",
                    );
                };
                let args = last.args.clone().unwrap_or_default();
                let mut call = e.clone();
                if let Ex::Post(_, segs) = &mut call {
                    let last = segs.last_mut().expect("checked");
                    last.args = Some(
                        (0..args.len())
                            .map(|i| Ex::Post(Head::Lit(format!("__a{i}")), Vec::new()))
                            .collect(),
                    );
                }
                let call = call
                    .emit(Scope::Template, true)
                    .map_err(|m| format!("line {line}: {m}"))?;
                if args.is_empty() {
                    return Ok(format!("let _ = index; {call};"));
                }
                let items = items.expect("template has items");
                let cloned = args
                    .iter()
                    .map(|a| {
                        a.emit(Scope::Template, false)
                            .map(|s| format!("::std::clone::Clone::clone(&({s}))"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|m| format!("line {line}: {m}"))?;
                let names: Vec<String> = (0..args.len()).map(|i| format!("__a{i}")).collect();
                Ok(format!(
                    "let __args = ::std::iter::IntoIterator::into_iter({items}).enumerate().nth(index as usize).map(|(index, item)| {{ let _ = (&index, &item); ({},) }});\n                if let ::std::option::Option::Some(({},)) = __args {{ {call}; }}",
                    cloned.join(", "),
                    names.join(", ")
                ))
            }
        }
    }

    /// Compiles `el` at `path` in `ctx`; returns the static element.
    fn element(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        path: &[u16],
        items: Option<&str>,
    ) -> Result<Element, String> {
        let el = &self.resolve(el)?;
        let name = el.name.as_str();
        let (kind, horizontal) = match name {
            "ToggleButton" => ("Button", false),
            "ListView" => ("ScrollView", false),
            "ItemsControl" => (
                "Column",
                el.attr("orientation")
                    .is_some_and(|o| o.trim() == "horizontal"),
            ),
            "ContentPresenter" => ("Column", false),
            "Screen.Resources" | "DataTemplate" => return err(el, "isn't allowed here"),
            k => (k, false),
        };
        let kind = if horizontal { "Row" } else { kind };
        let is_list = matches!(name, "ListView" | "ItemsControl" | "ContentPresenter");
        let screen = ctx.scope == Scope::Screen;

        let mut out = Element {
            name: kind.to_string(),
            attrs: Vec::new(),
            children: Vec::new(),
            line: el.line,
        };
        let mut raw = 0;
        let mut push_raw = |out: &mut Element, call: String| {
            out.attrs.push((format!("{RAW_CALL}{raw}"), call));
            raw += 1;
        };
        let mut patches: Vec<(&'static str, Vec<String>)> = Vec::new();
        let mut bindings: Vec<(String, Binding)> = Vec::new();
        let mut command: Option<Binding> = None;
        let mut list_attrs: HashMap<String, String> = HashMap::new();

        for (k, v) in &el.attrs {
            let k = k.as_str();
            if k.starts_with("d:") || matches!(k, "x:ViewModel" | "x:Create" | "x:DataType") {
                continue;
            }
            if matches!(k, "on_click" | "on_change" | "on_submit") {
                return err(
                    el,
                    format!(
                        "{k}: view-model screens use command=\"{{x:Bind method()}}\" (and Mode=TwoWay text) instead"
                    ),
                );
            }
            if let Some((state, attr)) = k.split_once(':') {
                let Some((_, flag)) = STATES.iter().find(|(s, _)| *s == state) else {
                    return err(
                        el,
                        format!(
                            "unknown attribute prefix `{state}:` (states: hover, pressed, focused, selected, checked, disabled)"
                        ),
                    );
                };
                let field = match attr {
                    "bg" | "color" | "focus_color" => {
                        let c = yui::color_attr(el, k, v)?;
                        format!("{attr}: ::std::option::Option::Some(0x{c:08X})")
                    }
                    "shadow" => format!(
                        "shadow: ::std::option::Option::Some({})",
                        yui::boolean(el, k, v)?
                    ),
                    _ => {
                        return err(
                            el,
                            format!(
                                "{k}: only paint attributes (bg, color, shadow, focus_color) take a state prefix"
                            ),
                        );
                    }
                };
                match patches.iter_mut().find(|(f, _)| f == flag) {
                    Some((_, fs)) => fs.push(field),
                    None => patches.push((flag, vec![field])),
                }
                continue;
            }
            if let Some(b) = parse_binding(v) {
                let b = b.map_err(|e| format!("line {}: {e}", el.line))?;
                if k == "command" {
                    command = Some(b);
                } else if matches!(k, "items" | "content" | "key" | "selected") {
                    if !is_list {
                        return err(
                            el,
                            format!("{k} is for ListView, ItemsControl and ContentPresenter"),
                        );
                    }
                    bindings.push((k.to_string(), b));
                } else {
                    bindings.push((k.to_string(), b));
                }
                continue;
            }
            match k {
                "command" => return err(el, "command needs {x:Bind method(...)}"),
                "items" | "content" | "key" | "selected" => {
                    return err(el, format!("{k} needs a binding: {k}=\"{{x:Bind ...}}\""));
                }
                "item_height" | "orientation" if is_list => {
                    list_attrs.insert(k.into(), v.clone());
                }
                "visible" => {
                    let b = yui::boolean(el, k, v)?;
                    push_raw(&mut out, format!("visible({b})"));
                }
                "checked" if kind == "Button" => {
                    let b = yui::boolean(el, k, v)?;
                    bindings.push((
                        k.into(),
                        Binding {
                            raw: v.clone(),
                            expr: Some(Ex::Post(Head::Lit(b.to_string()), Vec::new())),
                            rust: None,
                            mode: Mode::OneWay,
                            bind_back: None,
                            fallback: None,
                        },
                    ));
                }
                "text" | "placeholder"
                    if v.starts_with('@') && !v.starts_with("@@") && v.len() > 1 =>
                {
                    // Looked up on every update, so it follows the language.
                    bindings.push((
                        k.into(),
                        Binding {
                            raw: v.clone(),
                            expr: None,
                            rust: Some(yui::text_expr(v)),
                            mode: Mode::OneWay,
                            bind_back: None,
                            fallback: None,
                        },
                    ));
                }
                _ => out.attrs.push((k.to_string(), v.clone())),
            }
        }
        for (flag, fields) in patches {
            push_raw(
                &mut out,
                format!(
                    "state_style(::yog_api::ui::StateFlags::{flag}, ::yog_api::ui::StylePatch {{ {}, ..::std::default::Default::default() }})",
                    fields.join(", ")
                ),
            );
        }
        if let Some(id) = out.attr("id") {
            self.ids.push(id);
        }

        // Interactive parts (screen scope only).
        let text_box = if kind == "TextBox" && screen {
            let n = self.n_text_boxes;
            self.n_text_boxes += 1;
            let _ = writeln!(self.parts, "        parts.text_box({});", path_lit(path));
            Some(n)
        } else {
            None
        };
        let scroller = if kind == "ScrollView" {
            if !screen {
                return err(el, "scroll views inside item templates aren't supported");
            }
            let n = self.n_scrollers;
            self.n_scrollers += 1;
            let _ = writeln!(self.parts, "        parts.scroller({});", path_lit(path));
            Some(n)
        } else {
            None
        };

        // Commands and two-way toggles → an action.
        let toggle_place = bindings
            .iter()
            .find(|(k, b)| k == "checked" && b.mode == Mode::TwoWay)
            .map(|(_, b)| b.clone());
        if kind == "TextBox" {
            let tb = text_box.ok_or_else(|| {
                format!(
                    "line {}: text boxes in item templates aren't supported",
                    el.line
                )
            })?;
            if let Some((_, b)) = bindings.iter().find(|(k, _)| k == "text") {
                let write = match (&b.bind_back, b.mode, &b.expr) {
                    (Some(m), _, _) => Some(format!("vm.{m}(text);")),
                    (None, Mode::TwoWay, Some(e)) if e.is_place() => {
                        Some(format!("{} = text;", e.emit(Scope::Screen, false)?))
                    }
                    (None, Mode::TwoWay, _) => {
                        return err(
                            el,
                            "Mode=TwoWay needs a field (text=\"{x:Bind search, Mode=TwoWay}\") or BindBack=method",
                        );
                    }
                    _ => None,
                };
                if let Some(write) = write {
                    self.dispatch.push((
                        format!("::yog_kit::ViewEvent::TextChanged {{ slot: {tb}, text }}"),
                        write,
                    ));
                }
            }
            if let Some(c) = &command {
                let stmt = self.command(el, c, Scope::Screen, None)?;
                self.dispatch.push((
                    format!("::yog_kit::ViewEvent::TextSubmitted {{ slot: {tb}, text }}"),
                    format!("let _ = text; {stmt}"),
                ));
            }
        } else if command.is_some() || toggle_place.is_some() {
            let owner = ctx.owner;
            let slot = self.next_cmd(owner);
            push_raw(
                &mut out,
                format!(
                    "action(::yog_api::ui::Action {{ owner: {owner}, slot: {slot}, index: 0 }})"
                ),
            );
            let mut body = String::new();
            if let Some(t) = &toggle_place {
                if ctx.scope == Scope::Template {
                    return err(
                        el,
                        "Mode=TwoWay toggles in item templates aren't supported — use a command",
                    );
                }
                match (&t.bind_back, &t.expr) {
                    (Some(m), Some(e)) => {
                        let cur = e.emit(Scope::Screen, false)?;
                        let _ = write!(body, "let __v = !{cur}; vm.{m}(__v); ");
                    }
                    (None, Some(e)) if e.is_place() => {
                        let p = e.emit(Scope::Screen, false)?;
                        let _ = write!(body, "{p} = !{p}; ");
                    }
                    _ => {
                        return err(
                            el,
                            "checked Mode=TwoWay needs a bool field or BindBack=method",
                        );
                    }
                }
            }
            if let Some(c) = &command {
                body.push_str(&self.command(el, c, ctx.scope, items)?);
            }
            let pat = if owner == 0 {
                format!(
                    "::yog_kit::ViewEvent::Action(::yog_api::ui::Action {{ owner: 0, slot: {slot}, .. }})"
                )
            } else {
                format!(
                    "::yog_kit::ViewEvent::Action(::yog_api::ui::Action {{ owner: {owner}, slot: {slot}, index }})"
                )
            };
            self.dispatch.push((pat, body.trim_end().to_string()));
            if screen {
                let _ = writeln!(self.parts, "        parts.focusable({});", path_lit(path));
            }
        }

        // Lists.
        if is_list {
            if !screen {
                return err(el, "lists inside item templates aren't supported");
            }
            self.list(ctx, el, &mut bindings, &list_attrs, path, scroller)?;
        }

        // A component's bound properties: when any changes, the instance is
        // rebuilt with the new values (then its other bindings re-applied).
        let rebuild = if yui::is_custom_component(kind)
            && bindings.iter().any(|(k, _)| !yui::is_common_attr(k))
        {
            Some(self.component_props(ctx, el, &out, &mut bindings, path)?)
        } else {
            None
        };
        let slots_before = self.n_slots;
        if bindings.iter().any(|(k, _)| k == "source") {
            // A valid stand-in for the static checks; the binding sets it.
            match kind {
                "Image" => out.attrs.push(("source".into(), "item:air".into())),
                "Surface" => out.attrs.push(("source".into(), "yog:none".into())),
                _ => {}
            }
        }
        for (k, b) in &bindings {
            if matches!(k.as_str(), "items" | "content" | "key" | "selected") {
                continue;
            }
            self.bind_attr(ctx, el, kind, k, b, path, text_box, scroller)?;
        }
        if let Some(marker) = rebuild {
            let resets: String = (slots_before..self.n_slots)
                .map(|i| format!(" {}b{i}.reset();", ctx.fields))
                .collect();
            ctx.update = ctx.update.replace(&marker, &resets);
        }

        if !is_list {
            let mut i = 0u16;
            for child in &el.children {
                if child.name.contains('.') {
                    if child.name == "Screen.Resources" && path.is_empty() {
                        continue;
                    }
                    return err(child, "isn't allowed here");
                }
                let mut p = path.to_vec();
                p.push(i);
                out.children.push(self.element(ctx, child, &p, items)?);
                i += 1;
            }
        }
        Ok(out)
    }

    /// Binds the component properties among `bindings` (removing them):
    /// one text slot each, and an update block that rebuilds the instance
    /// at `path` from `out` (its static attributes) plus the slots' values.
    /// Returns the marker the block leaves for the resets of the element's
    /// other bindings.
    fn component_props(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        out: &Element,
        bindings: &mut Vec<(String, Binding)>,
        path: &[u16],
    ) -> Result<String, String> {
        let mut inst = out.clone();
        let mut sets = String::new();
        let mut i = 0;
        while i < bindings.len() {
            if yui::is_common_attr(&bindings[i].0) {
                i += 1;
                continue;
            }
            let (key, b) = bindings.remove(i);
            if b.mode == Mode::TwoWay || b.bind_back.is_some() {
                return err(el, format!("{key}: component properties bind one way"));
            }
            let expr = b.rust(ctx.scope, el)?;
            let value = if b.expr.as_ref().is_some_and(Ex::has_opt) {
                let fallback = b.fallback.clone().unwrap_or_default();
                format!(
                    "(|| ::std::option::Option::Some(::yog_kit::bind::text(&({expr}))))().unwrap_or_else(|| ::std::string::String::from({fallback:?}))"
                )
            } else {
                format!("::yog_kit::bind::text(&({expr}))")
            };
            let slot = self.slot(ctx, "String");
            let f = format!("{}{slot}", ctx.fields);
            let _ = write!(
                sets,
                "        // @yui {}:{} {key}=\"{}\"\n        __props |= {f}.set({value});\n",
                self.file,
                el.line,
                b.raw.replace('\n', " ")
            );
            inst.attrs.push((key, format!("{RAW_VALUE}*{f}.get()")));
        }
        let (expr, _) = yui::static_widget(&inst, false, 2, self.mod_id)?;
        let marker = format!("\u{1}resets{}\u{1}", self.n_slots);
        let _ = write!(
            ctx.update,
            "        let mut __props = false;\n{sets}        if __props {{\n            #[allow(unused_imports)]\n            {UI_USE}\n            *{}.at_path_mut({}) = {expr};\n            u.relayout();{marker}\n        }}\n",
            ctx.tree,
            path_lit(path),
        );
        Ok(marker)
    }

    /// The single element inside a template/empty property element.
    fn only_child(el: &Element) -> Result<&Element, String> {
        let mut c = el;
        if c.children.len() == 1 && c.children[0].name == "DataTemplate" {
            c = &c.children[0];
        }
        match c.children.as_slice() {
            [one] => Ok(one),
            _ => err(el, "needs exactly one child element"),
        }
    }

    fn list(
        &mut self,
        ctx: &mut Ctx,
        el: &Element,
        bindings: &mut [(String, Binding)],
        attrs: &HashMap<String, String>,
        path: &[u16],
        scroller: Option<u16>,
    ) -> Result<(), String> {
        let name = el.name.as_str();
        let owner = {
            self.n_lists += 1;
            self.n_lists
        };
        let get = |k: &str| {
            bindings
                .iter()
                .find(|(bk, _)| bk == k)
                .map(|(_, b)| b.clone())
        };
        let source = if name == "ContentPresenter" {
            "content"
        } else {
            "items"
        };
        let Some(items_b) = get(source) else {
            return err(el, format!("needs {source}=\"{{x:Bind ...}}\""));
        };
        let items_ex = items_b.expr.clone().expect("parsed");
        let mut items = items_ex
            .emit(Scope::Screen, false)
            .map_err(|e| format!("line {}: {e}", el.line))?;
        if items_ex.is_place() {
            items = format!("&{items}");
        }
        let key = match get("key") {
            Some(b) => b
                .expr
                .expect("parsed")
                .emit(Scope::Template, false)
                .map_err(|e| format!("line {}: {e}", el.line))?,
            None => "index".to_string(),
        };
        let selected = get("selected");
        if selected.is_some() && name != "ListView" {
            return err(el, "selected is for ListView");
        }
        let item_height = match attrs.get("item_height") {
            Some(v) => Some(yui::number(el, "item_height", v)?),
            None => None,
        };

        // Templates.
        let mut template = None;
        let mut empty = None;
        for c in &el.children {
            match c.name.strip_prefix(name).and_then(|n| n.strip_prefix('.')) {
                Some("ItemTemplate" | "ContentTemplate") => template = Some(Self::only_child(c)?),
                Some("Empty") => empty = Some(Self::only_child(c)?),
                _ if c.name == "DataTemplate" => template = Some(Self::only_child(c)?),
                _ => {
                    return err(
                        c,
                        format!(
                            "isn't allowed in <{name}> (expected <{name}.ItemTemplate> or <{name}.Empty>)"
                        ),
                    );
                }
            }
        }
        let Some(template) = template else {
            return err(
                el,
                format!("needs an item template (<{name}.ItemTemplate><DataTemplate>...)"),
            );
        };

        let t = self.n_templates;
        self.n_templates += 1;
        let mut tctx = Ctx::new(Scope::Template, "inst.", "w", owner);
        let saved = std::mem::take(&mut self.n_slots);
        let tel = self.element(&mut tctx, template, &[], Some(&items))?;
        self.n_slots = saved;
        let (texpr, ids) = yui::static_widget(&tel, false, 1, self.mod_id)?;
        self.absorb(ids);
        let fields: String = tctx
            .slots
            .iter()
            .map(|(n, ty)| format!("    {n}: ::yog_kit::bind::Slot<{ty}>,\n"))
            .collect();
        let inits: String = tctx
            .slots
            .iter()
            .map(|(n, _)| format!("{n}: ::yog_kit::bind::Slot::new(), "))
            .collect();
        let _ = write!(
            self.templates,
            "/// Item template {t} ({}:{}).\n#[doc(hidden)]\n#[allow(non_camel_case_types)]\nstruct __T{t} {{\n{fields}}}\n\nimpl __T{t} {{\n    fn build() -> (Self, ::yog_api::ui::Widget) {{\n        #[allow(unused_imports)]\n        {UI_USE}\n        (Self {{ {inits}}}, {texpr})\n    }}\n}}\n\n",
            self.file, template.line
        );

        // Empty state: built once, bound in view scope.
        let mut empty_update = String::new();
        let empty_fn = match empty {
            Some(e) => {
                let mut ectx = Ctx::new(Scope::Screen, "self.", "w", 0);
                let eel = self.element(&mut ectx, e, &[], None)?;
                let (eexpr, ids) = yui::static_widget(&eel, false, 1, self.mod_id)?;
                self.absorb(ids);
                empty_update = ectx.update;
                let _ = write!(
                    self.templates,
                    "#[doc(hidden)]\n#[allow(non_snake_case)]\nfn __E{t}() -> ::yog_api::ui::Widget {{\n    #[allow(unused_imports)]\n    {UI_USE}\n    {eexpr}\n}}\n\n"
                );
                format!("::std::option::Option::Some(__E{t})")
            }
            None => "::std::option::Option::None".to_string(),
        };

        let ih = item_height.map_or("::std::option::Option::None".to_string(), |h| {
            format!("::std::option::Option::Some({h:?})")
        });
        let sc = scroller.map_or("::std::option::Option::None".to_string(), |s| {
            format!("::std::option::Option::Some({s})")
        });
        let _ = writeln!(
            self.parts,
            "        parts.list({owner}, {}, {sc}, {ih});",
            path_lit(path)
        );
        let field = format!("l{owner}");
        self.view_fields.push((
            field.clone(),
            format!("::yog_kit::items::ItemsHost<__T{t}>"),
            format!(
                "::yog_kit::items::ItemsHost::new({owner}, {}, {ih}, {sc}, {})",
                path_lit(path),
                selected.is_some()
            ),
        ));

        // Update.
        let mut u = String::new();
        let _ = writeln!(
            u,
            "        // @yui {}:{} {source}=\"{}\"",
            self.file, el.line, items_b.raw
        );
        u.push_str("        {\n");
        let sel_expr = match &selected {
            Some(b) => {
                let e = b
                    .expr
                    .as_ref()
                    .expect("parsed")
                    .emit(Scope::Screen, false)?;
                let _ = writeln!(u, "            let __sel = {e};");
                format!("::yog_kit::bind::is_selected(&__sel, &({key}))")
            }
            None => "false".into(),
        };
        let _ = writeln!(u, "            let mut __it = self.{field}.begin(root, u);");
        let _ = writeln!(
            u,
            "            for (index, item) in ::std::iter::IntoIterator::into_iter({items}).enumerate() {{"
        );
        u.push_str("                let _ = (&index, &item);\n");
        if tctx.update.is_empty() {
            let _ = writeln!(
                u,
                "                let _ = __it.next(::yog_kit::bind::key(&({key})), {sel_expr}, __T{t}::build);"
            );
        } else {
            let _ = writeln!(
                u,
                "                let ::std::option::Option::Some(__row) = __it.next(::yog_kit::bind::key(&({key})), {sel_expr}, __T{t}::build) else {{ continue }};"
            );
            u.push_str("                let ::yog_kit::items::Item { inst, w, u } = __row;\n");
            for line in tctx.update.lines() {
                let _ = writeln!(u, "        {line}");
            }
        }
        u.push_str("            }\n");
        if empty_update.is_empty() {
            let _ = writeln!(u, "            __it.finish({empty_fn});");
        } else {
            let _ = writeln!(
                u,
                "            if let ::std::option::Option::Some((w, u)) = __it.finish({empty_fn}) {{"
            );
            for line in empty_update.lines() {
                let _ = writeln!(u, "        {line}");
            }
            u.push_str("            }\n");
        }
        u.push_str("        }\n");
        ctx.update.push_str(&u);

        // Selection write-back.
        if let Some(b) = &selected {
            let write = match (&b.bind_back, b.mode, &b.expr) {
                (Some(m), _, _) => format!("vm.{m}(__k);"),
                (None, Mode::TwoWay, Some(e)) if e.is_place() => {
                    format!(
                        "{} = ::std::option::Option::Some(__k);",
                        e.emit(Scope::Screen, false)?
                    )
                }
                (None, Mode::TwoWay, _) => {
                    return err(el, "selected Mode=TwoWay needs a field or BindBack=method");
                }
                _ => String::new(),
            };
            if !write.is_empty() {
                self.dispatch.push((
                    format!(
                        "::yog_kit::ViewEvent::Action(::yog_api::ui::Action {{ owner: {owner}, slot: ::yog_kit::items::SELECT, index }})\n            | ::yog_kit::ViewEvent::Select {{ list: {owner}, index }}"
                    ),
                    format!(
                        "let __k = ::std::iter::IntoIterator::into_iter({items}).enumerate().nth(index as usize).map(|(index, item)| {{ let _ = (&index, &item); ::std::clone::Clone::clone(&({key})) }});\n                if let ::std::option::Option::Some(__k) = __k {{ {write} }}"
                    ),
                ));
            }
        }
        // A list's own bindings other than items/key/selected (bg, w, ...)
        // are applied by the caller.
        Ok(())
    }

    fn absorb(&mut self, ids: Vec<String>) {
        for id in &ids {
            self.ids.push(id);
        }
    }
}

/// Generates a view-model screen's `.g.rs` (see the module docs).
pub fn generate(
    root: &Element,
    name: &str,
    mod_id: Option<&str>,
) -> Result<(String, Vec<String>), String> {
    if root.name != "Screen" {
        return err(root, "a view-model screen's root must be <Screen>");
    }
    let id = root
        .attr("id")
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "line {}: <Screen> needs an id (the UI id, e.g. \"mymod:main\")",
                root.line
            )
        })?
        .to_string();
    let vm = root
        .attr("x:ViewModel")
        .unwrap_or_default()
        .trim()
        .to_string();
    if vm.is_empty() {
        return err(root, "x:ViewModel names the view-model type");
    }
    let create = root
        .attr("x:Create")
        .map(str::to_string)
        .unwrap_or_else(|| format!("<{vm} as ::std::default::Default>::default()"));
    let (width, height, anchor) = yui::screen_frame(root)?;
    let anchor = yui::pascal(anchor);
    let modal = root
        .attr("modal")
        .map(|v| yui::boolean(root, "modal", v))
        .transpose()?
        .unwrap_or(true);
    let pause = root
        .attr("pause")
        .map(|v| yui::boolean(root, "pause", v))
        .transpose()?
        .unwrap_or(false);

    let mut g = VmGen {
        file: format!("{name}.yui"),
        mod_id,
        resources: HashMap::new(),
        styles: HashMap::new(),
        ids: yui::Names::default(),
        view_fields: Vec::new(),
        parts: String::new(),
        dispatch: Vec::new(),
        templates: String::new(),
        n_slots: 0,
        n_lists: 0,
        n_templates: 0,
        n_scrollers: 0,
        n_text_boxes: 0,
        cmd_slots: HashMap::new(),
    };
    for c in &root.children {
        if c.name == "Screen.Resources" {
            g.collect_resources(c)?;
        }
    }
    let mut content = root.clone();
    content.attrs.retain(|(k, _)| k != "id");
    let mut ctx = Ctx::new(Scope::Screen, "self.", "root", 0);
    let tree = g.element(&mut ctx, &content, &[], None)?;
    let (expr, ids) = yui::static_widget(&tree, false, 0, mod_id)?;
    g.absorb(ids);

    let mut out = String::new();
    let _ = write!(
        out,
        r#"// <auto-generated>
//     Generated by yog from src/{name}.yui (view model {vm}, codegen {CODEGEN}) —
//     do not edit: changes are lost on the next `yog build` / `yog check` /
//     `yog ui gen`. Edit the .yui file instead.
// </auto-generated>
//
// Pulled into the code-behind module (which defines `{vm}`) with:
//
//     yog_api::ui_include!("{name}");

const _: () = ::yog_kit::__codegen::require({CODEGEN});

/// The UI id this screen registers under.
#[allow(dead_code)]
pub const ID: &str = {id:?};
/// Design-time size in GUI pixels.
#[allow(dead_code)]
pub const WIDTH: f32 = {width:?};
#[allow(dead_code)]
pub const HEIGHT: f32 = {height:?};

/// Ids of this screen's named widgets.
#[allow(dead_code)]
pub mod ids {{
"#
    );
    out.push_str(&yui::str_consts(g.ids.as_slice()));
    out.push_str("}\n\n");
    out.push_str(&g.templates);

    // View struct.
    out.push_str("/// The compiled view of this screen — hosted by `yog_kit::Screen`.\n/// Private like the view model it names; tests in the code-behind\n/// module drive it with `yog_kit::testing::TestHost<View>`.\nstruct View {\n");
    for (n, ty, _) in &g.view_fields {
        let _ = writeln!(out, "    {n}: {ty},");
    }
    out.push_str("}\n\n");

    let _ = write!(
        out,
        r#"impl ::yog_kit::View for View {{
    type Vm = {vm};
    const META: ::yog_kit::ScreenMeta = ::yog_kit::ScreenMeta {{
        id: ID,
        w: WIDTH,
        h: HEIGHT,
        anchor: ::yog_kit::Anchor::{anchor},
        modal: {modal},
        pause: {pause},
    }};

    fn build() -> (Self, ::yog_api::ui::Widget, ::yog_kit::Parts) {{
        #[allow(unused_imports)]
        {UI_USE}
        let root = {expr};
        #[allow(unused_mut)]
        let mut parts = ::yog_kit::Parts::new();
{parts}        let view = View {{
"#,
        parts = g.parts,
    );
    for (n, _, init) in &g.view_fields {
        let _ = writeln!(out, "            {n}: {init},");
    }
    let _ = write!(
        out,
        r#"        }};
        (view, root, parts)
    }}

    #[allow(unused_variables)]
    fn update(&mut self, root: &mut ::yog_api::ui::Widget, vm: &{vm}, u: &mut ::yog_kit::Update) {{
{update}    }}

    #[allow(unused_variables, clippy::let_unit_value, clippy::single_match, clippy::match_single_binding)]
    fn dispatch(&mut self, vm: &mut {vm}, cx: &mut ::yog_kit::Cx<'_, {vm}>, ev: ::yog_kit::ViewEvent) {{
        match ev {{
"#,
        update = ctx.update,
    );
    for (pat, body) in &g.dispatch {
        let _ = writeln!(
            out,
            "            {pat} => {{\n                {body}\n            }}"
        );
    }
    let _ = write!(
        out,
        r#"            _ => {{}}
        }}
    }}
}}

static __HANDLE: ::std::sync::OnceLock<::yog_kit::ScreenHandle<{vm}>> = ::std::sync::OnceLock::new();

/// Registers the screen (rendering + input) with a new view model
/// (`{create}`); returns its handle. Call once, from `Mod::register`.
#[allow(dead_code, private_interfaces)]
pub fn register(registry: &mut ::yog_api::Registry) -> ::yog_kit::ScreenHandle<{vm}> {{
    register_with(registry, {create})
}}

/// [`register`] with a given view model.
#[allow(dead_code, private_interfaces)]
pub fn register_with(registry: &mut ::yog_api::Registry, vm: {vm}) -> ::yog_kit::ScreenHandle<{vm}> {{
    ::std::clone::Clone::clone(__HANDLE.get_or_init(|| ::yog_kit::Screen::<View>::register(registry, vm)))
}}

/// The handle [`register`] returned (`None` before it).
#[allow(dead_code, private_interfaces)]
pub fn handle() -> ::std::option::Option<::yog_kit::ScreenHandle<{vm}>> {{
    __HANDLE.get().cloned()
}}

/// Opens the screen (modal: {modal}, pauses the game: {pause}).
#[allow(dead_code)]
pub fn open() {{
    ::yog_api::open_ui(ID, {modal}, {pause});
}}

/// Closes the screen if it's open.
#[allow(dead_code)]
pub fn close() {{
    ::yog_api::close_ui(ID);
}}
"#
    );
    Ok((out, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codegen(body: &str) -> Result<String, String> {
        let src = format!(
            r#"<Screen id="t:s" width="200" height="100" x:ViewModel="Vm">{body}</Screen>"#
        );
        crate::yui::generate_for(&src, "s", None).map(|(c, _)| c)
    }

    #[test]
    fn expressions() {
        let e = |s: &str, scope| parse_expr(s).unwrap().emit(scope, false).unwrap();
        assert_eq!(e("count", Scope::Screen), "vm.count");
        assert_eq!(e("count_text()", Scope::Screen), "vm.count_text()");
        assert_eq!(
            e("filter == Filter::All", Scope::Screen),
            "vm.filter == Filter::All"
        );
        assert_eq!(
            e("!has(Filter::Yog)", Scope::Screen),
            "!vm.has(Filter::Yog)"
        );
        assert_eq!(
            e("Strings::count(n, 'x')", Scope::Screen),
            "Strings::count(&vm.n, \"x\")"
        );
        assert_eq!(
            e("Strings::v(version)", Scope::Template),
            "Strings::v(&item.version)"
        );
        assert_eq!(e("selected()?.name", Scope::Screen), "vm.selected()?.name");
        assert_eq!(e("vm.x.len()", Scope::Template), "vm.x.len()");
        assert_eq!(e("item", Scope::Template), "item");
        assert_eq!(e("1.5", Scope::Screen), "1.5");
        assert!(parse_expr("a < b").is_err());
        assert!(
            parse_expr("cx.close()")
                .unwrap()
                .emit(Scope::Screen, false)
                .is_err()
        );
        assert!(
            parse_expr("index")
                .unwrap()
                .emit(Scope::Screen, false)
                .is_err()
        );
    }

    #[test]
    fn yres_colors_everywhere() {
        let mut refs = crate::yres::Refs::default();
        refs.colors.insert("Colors.accent".into(), 0xFF11_2233);
        refs.colors.insert("Colors.hot".into(), 0xFF44_5566);
        refs.colors.insert("Colors.tint".into(), 0x8077_8899);
        crate::yui::set_resource_refs(&refs);
        let code = codegen(
            r##"
            <Screen.Resources>
                <Color x:Key="Accent" value="@Colors.accent"/>
                <Style x:Key="Btn" target="Button" bg="@Colors.accent" hover:bg="@Colors.hot"/>
            </Screen.Resources>
            <Column w="100%">
                <Label text="a" color="@Accent" selected:color="@Accent" disabled:bg="@Colors.hot"/>
                <Button style="@Btn" text="b"/>
                <Image source="item:air" tint="@Colors.tint"/>
            </Column>"##,
        );
        let missing = codegen(r#"<Label text="a" hover:bg="@Colors.nope"/>"#);
        crate::yui::set_resource_refs(&crate::yres::Refs::default());
        let code = code.unwrap();
        for want in [
            ".color(0xFF112233)",
            "StateFlags::SELECTED, ::yog_api::ui::StylePatch { color: ::std::option::Option::Some(0xFF112233)",
            "StateFlags::DISABLED, ::yog_api::ui::StylePatch { bg: ::std::option::Option::Some(0xFF445566)",
            ".bg(0xFF112233)",
            "StateFlags::HOVER, ::yog_api::ui::StylePatch { bg: ::std::option::Option::Some(0xFF445566)",
            ".tint(0x80778899)",
        ] {
            assert!(code.contains(want), "missing {want}:\n{code}");
        }
        assert!(missing.unwrap_err().contains("no color `Colors.nope`"));
    }

    #[test]
    fn bindings_commands_and_parts() {
        let code = codegen(r##"
            <Screen.Resources>
                <Color x:Key="Accent" value="#FFD700"/>
                <Style x:Key="Btn" target="Button" w="40" h="14" bg="#FF333333" hover:bg="#FF444444"/>
                <Style x:Key="Toggle" based_on="@Btn" checked:bg="#FF555555"/>
            </Screen.Resources>
            <Column w="100%">
                <Label x:Name="count" text="{x:Bind count_text()}" color="@Accent"/>
                <Label text="@mymod.title"/>
                <Button style="@Btn" text="Close" command="{x:Bind cx.close()}"/>
                <ToggleButton style="@Toggle" text="Yog" checked="{x:Bind filter == Filter::Yog}"
                              enabled="{x:Bind has(Filter::Yog)}" command="{x:Bind set_filter(Filter::Yog)}"/>
                <TextBox w="100" h="14" text="{x:Bind search, Mode=TwoWay}" placeholder="@mymod.search"
                         command="{x:Bind submit()}"/>
                <ScrollView h="50" scroll_key="{x:Bind search}" visible="{x:Bind show}"/>
            </Column>"##)
        .unwrap();
        for want in [
            "const _: () = ::yog_kit::__codegen::require(1);",
            "type Vm = Vm;",
            ".color(0xFFFFD700)",
            r#".id("count")"#,
            "pub const COUNT: &str = \"count\";",
            "if self.b0.set(::yog_kit::bind::text(&(vm.count_text()))) { root.at_path_mut(&[0, 0]).set_text(",
            r#"::yog_kit::bind::text(&(::yog_api::tr("mymod.title")))"#,
            ".state_style(::yog_api::ui::StateFlags::HOVER, ::yog_api::ui::StylePatch { bg: ::std::option::Option::Some(0xFF444444), ..::std::default::Default::default() })",
            ".state_style(::yog_api::ui::StateFlags::CHECKED",
            ".action(::yog_api::ui::Action { owner: 0, slot: 0, index: 0 })",
            ".action(::yog_api::ui::Action { owner: 0, slot: 1, index: 0 })",
            "::yog_kit::bind::set_checked(root.at_path_mut(&[0, 3]), *self.",
            "::yog_kit::bind::flag(vm.filter == Filter::Yog)",
            "slot: 0, .. }) => {\n                cx.close();",
            "slot: 1, .. }) => {\n                vm.set_filter(Filter::Yog);",
            "parts.focusable(&[0, 2]);",
            "parts.text_box(&[0, 4]);",
            "parts.scroller(&[0, 5]);",
            "u.set_text_box(0, self.",
            "::yog_kit::ViewEvent::TextChanged { slot: 0, text } => {\n                vm.search = text;",
            "::yog_kit::ViewEvent::TextSubmitted { slot: 0, text } => {\n                let _ = text; vm.submit();",
            "u.reset_scroll(0);",
            ".visible = *self.",
            "::yog_kit::Screen::<View>::register(registry, vm)",
            "<Vm as ::std::default::Default>::default()",
        ] {
            assert!(code.contains(want), "missing {want}\n{code}");
        }
        assert!(
            !code.contains("text=") || code.contains("// @yui"),
            "{code}"
        );
    }

    #[test]
    fn bound_sources() {
        let code = codegen(
            r##"
            <Column>
                <Surface w="10" h="10" source="{x:Bind grid_id()}"/>
                <Image w="10" h="10" source="{x:Bind icon}"/>
            </Column>"##,
        )
        .unwrap();
        for want in [
            r#"widget::surface("yog:none")"#,
            r#"widget::image("item:air")"#,
            "::yog_kit::bind::set_source(root.at_path_mut(&[0, 0]), ",
            "::yog_kit::bind::set_source(root.at_path_mut(&[0, 1]), ",
        ] {
            assert!(code.contains(want), "missing {want}\n{code}");
        }
    }

    #[test]
    fn component_properties() {
        let code = codegen(r##"
            <Column>
                <StatBar id="hp" w="80" label="HP" value="{x:Bind hp}" fill="{x:Bind color()}" visible="{x:Bind alive}"/>
                <ListView h="40" items="{x:Bind rows}" key="{x:Bind id}">
                    <ListView.ItemTemplate>
                        <DataTemplate>
                            <Pill text="{x:Bind name}"/>
                        </DataTemplate>
                    </ListView.ItemTemplate>
                </ListView>
            </Column>"##)
        .unwrap();
        for want in [
            "__props |= self.b0.set(::yog_kit::bind::text(&(vm.hp)));",
            "__props |= self.b1.set(::yog_kit::bind::text(&(vm.color())));",
            r#"::yog_api::ui::component::build("StatBar", &[("label", "HP"), ("value", &**self.b0.get()), ("fill", &**self.b1.get())])"#,
            "u.relayout(); self.b2.reset();",
            r#"::yog_api::ui::component::build("Pill", &[("text", &**inst."#,
        ] {
            assert!(code.contains(want), "missing {want}\n{code}");
        }
        assert!(codegen(r#"<StatBar value="{x:Bind hp, Mode=TwoWay}"/>"#).is_err());
    }

    #[test]
    fn lists() {
        let code = codegen(
            r##"
            <ListView w="100%" h="80" items="{x:Bind visible()}" key="{x:Bind id}"
                      selected="{x:Bind selected_id(), BindBack=select}" item_height="12">
                <ListView.ItemTemplate>
                    <DataTemplate x:DataType="Entry">
                        <Row w="100%" selected:bg="#FF333366">
                            <Label text="{x:Bind name}"/>
                            <Button text="x" command="{x:Bind vm.remove(id)}"/>
                        </Row>
                    </DataTemplate>
                </ListView.ItemTemplate>
                <ListView.Empty>
                    <Label text="{x:Bind empty_text()}"/>
                </ListView.Empty>
            </ListView>
            <ContentPresenter content="{x:Bind selected()}">
                <DataTemplate><Label text="{x:Bind Strings::v(version)}"/></DataTemplate>
            </ContentPresenter>"##,
        )
        .unwrap();
        for want in [
            "struct __T0 {",
            "fn __E0() -> ::yog_api::ui::Widget",
            "l1: ::yog_kit::items::ItemsHost<__T0>,",
            "::yog_kit::items::ItemsHost::new(1, &[0], ::std::option::Option::Some(12.0), ::std::option::Option::Some(0), true)",
            "parts.list(1, &[0], ::std::option::Option::Some(0), ::std::option::Option::Some(12.0));",
            "let __sel = vm.selected_id();",
            "for (index, item) in ::std::iter::IntoIterator::into_iter(vm.visible()).enumerate() {",
            "__it.next(::yog_kit::bind::key(&(item.id)), ::yog_kit::bind::is_selected(&__sel, &(item.id)), __T0::build)",
            "if inst.b0.set(::yog_kit::bind::text(&(item.name))) { w.at_path_mut(&[0]).set_text(",
            "if let ::std::option::Option::Some((w, u)) = __it.finish(::std::option::Option::Some(__E0)) {",
            "self.b0.set(::yog_kit::bind::text(&(vm.empty_text())))",
            ".action(::yog_api::ui::Action { owner: 1, slot: 0, index: 0 })",
            "::yog_kit::ViewEvent::Action(::yog_api::ui::Action { owner: 1, slot: 0, index }) => {",
            "::std::clone::Clone::clone(&(item.id))",
            "if let ::std::option::Option::Some((__a0,)) = __args { vm.remove(__a0); }",
            "| ::yog_kit::ViewEvent::Select { list: 1, index } => {",
            "vm.select(__k);",
            "::yog_kit::items::ItemsHost::new(2, &[1], ::std::option::Option::None, ::std::option::Option::None, false)",
            "into_iter(vm.selected()).enumerate()",
            "Strings::v(&item.version)",
            "__it.finish(::std::option::Option::None);",
        ] {
            assert!(code.contains(want), "missing {want}\n{code}");
        }
    }

    #[test]
    fn errors() {
        let e = |b: &str| codegen(b).unwrap_err();
        assert!(e(r#"<Button on_click="x"/>"#).contains("command="),);
        assert!(e(r#"<Label w="{x:Bind a}" hover:w="3"/>"#).contains("paint attributes"));
        assert!(e(r#"<TextBox text="{x:Bind name(), Mode=TwoWay}"/>"#).contains("TwoWay"));
        assert!(e(r#"<Label text="{x:Bind a &lt; b}"/>"#).contains("unexpected"));
        assert!(e(r#"<Label color="@Nope"/>"#).contains("no resource"));
        assert!(e(r#"<ListView items="{x:Bind xs}"/>"#).contains("item template"));
        assert!(e(r#"<Label text="{x:Bind index}"/>"#).contains("item templates"));
    }

    /// The yog-kit fixture's committed `.g.rs` matches the generator
    /// (`UPDATE_GOLDEN=1` rewrites it); yog-kit compiles and runs it.
    #[test]
    fn kit_fixture() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../yog-kit/tests/fixtures");
        for name in ["list", "grid"] {
            let src = std::fs::read_to_string(dir.join(format!("{name}.yui"))).unwrap();
            let (code, warnings) = crate::yui::generate_for(&src, name, None).unwrap();
            assert!(warnings.is_empty(), "{warnings:?}");
            let out = dir.join(format!("{name}.g.rs"));
            if std::env::var_os("UPDATE_GOLDEN").is_some() {
                std::fs::write(&out, &code).unwrap();
            }
            let want = std::fs::read_to_string(&out).unwrap_or_default();
            assert!(
                want == code,
                "{} is stale; rerun with UPDATE_GOLDEN=1",
                out.display()
            );
        }
    }

    #[test]
    fn legacy_screens_unchanged() {
        let src = r#"<Screen id="t:s"><Label text="@t.k" on_click="go"/></Screen>"#;
        let (code, _) = crate::yui::generate_for(src, "s", None).unwrap();
        assert!(code.contains("pub fn render(gfx"), "{code}");
        assert!(!code.contains("yog_kit"), "{code}");
    }
}
