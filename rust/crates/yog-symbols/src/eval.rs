//! Lazily expanded values and watch expressions, for a debugger's Watch,
//! Autos and Locals windows and editor DataTips.
//!
//! [`SymbolTable::local_nodes`] and [`SymbolTable::evaluate`] produce one
//! [`ValueNode`] per row — a one-line summary plus, for aggregates and
//! pointers, a [`ValueRef`] that [`SymbolTable::expand`] turns into the
//! next level of rows on demand, so a deep or wide value costs nothing
//! until the user opens it.
//!
//! The expression language is deliberately small and side-effect free:
//! variable names (locals and parameters of the frame, then statics by
//! name or `path::NAME`), field access `a.b`, tuple fields `a.0`,
//! dereference `*p`, indexing `a[3]` / `a[i]` into arrays, slices and
//! `Vec`s, and parentheses. Fields and indexing see through references,
//! smart pointers and std wrappers the way Rust's auto-deref does.

use std::cell::OnceCell;

use gimli::UnitOffset;

use crate::format::{short_type_name, Formatter, Slot, ELLIPSIS};
use crate::locals::{
    unit_ref, Frame, LocalVar, Located, NoMemory, NOT_IN_SCOPE, OPTIMIZED_OUT, UNAVAILABLE,
};
use crate::{FrameRegs, SymbolTable, TargetMemory};

/// Node budget for formatting one row's summary (whose children are
/// dropped — they're fetched with [`SymbolTable::expand`] instead).
const ROW_BUDGET: usize = 512;
/// Rows one [`SymbolTable::expand`] returns for a sequence.
const EXPAND_LIMIT: u64 = 1000;
/// Nesting of parentheses, derefs and indices in one watch expression.
const MAX_EXPR_NESTING: usize = 64;
/// Type shown for an integer literal.
const INTEGER_LITERAL_TYPE: &str = "{integer}";

/// A value in the stopped target that can be expanded with
/// [`SymbolTable::expand`]. Only meaningful while the target stays stopped
/// at the same place: it records where the value's bytes are, not the
/// bytes themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRef {
    unit: usize,
    slot: Slot,
}

/// One row of a lazily expanded variables view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueNode {
    /// Row label: the variable name, `field`, tuple index `0`, `[3]`,
    /// `*name` for a pointer's target — or the expression evaluated.
    pub name: String,
    /// An expression [`SymbolTable::evaluate`] accepts that names this
    /// value (e.g. `shape.0.x`), for "Add Watch" on a row.
    pub expr: String,
    pub type_name: String,
    /// One-line summary; placeholders like `<optimized out>` for a value
    /// that can't be read.
    pub value: String,
    /// Set when the row has children; pass to [`SymbolTable::expand`].
    pub children: Option<ValueRef>,
}

/// Why an expression couldn't be evaluated — shown in place of a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError(String);

impl EvalError {
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EvalError {}

impl ValueNode {
    /// A row without children or a readable value.
    fn leaf(name: String, expr: String, type_name: String, value: String) -> Self {
        ValueNode {
            name,
            expr,
            type_name,
            value,
            children: None,
        }
    }
}

/// A static variable (`DW_TAG_variable` outside any function).
pub(crate) struct GlobalVar {
    qualified: String,
    unit: usize,
    die: UnitOffset<usize>,
}

impl SymbolTable {
    /// [`SymbolTable::locals_in_frame`] at the frame's pc, each as a
    /// lazily expandable row (compare [`SymbolTable::read_locals_in_frame`],
    /// which formats a bounded tree up front).
    pub fn local_nodes(
        &self,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
        inline_depth: usize,
    ) -> Vec<ValueNode> {
        let pc = regs.pc().wrapping_sub(load_base);
        self.locals_in_frame(pc, inline_depth)
            .iter()
            .map(|var| match self.locate_local(var, regs, load_base, mem) {
                Ok(slot) => self.node(var.unit, slot, mem),
                Err(placeholder) => ValueNode::leaf(
                    var.name.clone(),
                    var.name.clone(),
                    short_type_name(&var.type_name),
                    placeholder.to_string(),
                ),
            })
            .collect()
    }

    /// Evaluates a watch expression (see the module docs for the
    /// language) in the frame described by `regs` / `inline_depth`, as
    /// for [`SymbolTable::local_nodes`]. The row's `name` is `expr` as
    /// given (trimmed).
    pub fn evaluate(
        &self,
        expr: &str,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
        inline_depth: usize,
    ) -> Result<ValueNode, EvalError> {
        let text = expr.trim();
        let ast = parse(text).map_err(EvalError)?;
        let ctx = Ctx {
            table: self,
            regs,
            load_base,
            mem,
            inline_depth,
            locals: OnceCell::new(),
        };
        match ctx.eval(&ast).map_err(EvalError)? {
            Val::Int(n) => Ok(ValueNode::leaf(
                text.to_string(),
                text.to_string(),
                INTEGER_LITERAL_TYPE.into(),
                n.to_string(),
            )),
            Val::Obj(unit, slot) => {
                let mut node = self.node(unit, slot, mem);
                node.name = text.to_string();
                Ok(node)
            }
        }
    }

    /// The rows under an expandable row, read from `mem` now (the target
    /// must still be stopped where `value` came from). Sequences are cut
    /// off after [`EXPAND_LIMIT`] elements, with a final `…` row.
    pub fn expand(&self, value: &ValueRef, mem: &dyn TargetMemory) -> Vec<ValueNode> {
        let Some(info) = self.units().get(value.unit) else {
            return Vec::new();
        };
        let fmt = Formatter::new(self, info, mem);
        let mut slots = fmt.child_slots(&value.slot, EXPAND_LIMIT + 1);
        let more = slots.len() as u64 > EXPAND_LIMIT;
        slots.truncate(EXPAND_LIMIT as usize);
        let mut out: Vec<ValueNode> = slots
            .into_iter()
            .map(|slot| self.node(value.unit, slot, mem))
            .collect();
        if more {
            out.push(ValueNode::leaf(
                ELLIPSIS.into(),
                String::new(),
                String::new(),
                String::new(),
            ));
        }
        out
    }

    fn node(&self, unit: usize, slot: Slot, mem: &dyn TargetMemory) -> ValueNode {
        let Some(info) = self.units().get(unit) else {
            return ValueNode::leaf(slot.name, slot.expr, String::new(), UNAVAILABLE.into());
        };
        let summary = Formatter::with_budget(self, info, mem, ROW_BUDGET).format(
            slot.name.clone(),
            slot.ty,
            &slot.place,
            0,
        );
        let expandable = !Formatter::new(self, info, mem)
            .child_slots(&slot, 1)
            .is_empty();
        ValueNode {
            name: slot.name.clone(),
            expr: slot.expr.clone(),
            type_name: summary.type_name,
            value: summary.value,
            children: expandable.then_some(ValueRef { unit, slot }),
        }
    }

    /// Where a local's bytes are, or the placeholder to show instead.
    fn locate_local(
        &self,
        var: &LocalVar,
        regs: &FrameRegs,
        load_base: u64,
        mem: &dyn TargetMemory,
    ) -> Result<Slot, &'static str> {
        if !var.in_scope {
            return Err(NOT_IN_SCOPE);
        }
        let info = self.units().get(var.unit).ok_or(UNAVAILABLE)?;
        let entry = info.unit.entry(var.die).map_err(|_| UNAVAILABLE)?;
        let frame = Frame::new(regs, mem, load_base);
        match self.locate(info, &entry, var.subprogram, &frame) {
            Located::Place(place) => Ok(Slot {
                name: var.name.clone(),
                expr: var.name.clone(),
                ty: var.ty,
                place,
            }),
            Located::OptimizedOut => Err(OPTIMIZED_OUT),
            Located::Unavailable => Err(UNAVAILABLE),
        }
    }

    /// Every static in the native, by qualified name (built once).
    fn globals(&self) -> &[GlobalVar] {
        self.globals.get_or_init(|| {
            let mut out = Vec::new();
            for (unit_idx, info) in self.units().iter().enumerate() {
                let fmt = Formatter::new(self, info, &NoMemory);
                // Tags of the open ancestors; statics live under the CU
                // and namespaces (or types), never inside a function.
                let mut stack: Vec<gimli::DwTag> = Vec::new();
                let mut cursor = info.unit.entries();
                while let Ok(Some(entry)) = cursor.next_dfs() {
                    let depth = entry.depth().max(0) as usize;
                    stack.truncate(depth);
                    let tag = entry.tag();
                    stack.push(tag);
                    if tag != gimli::DW_TAG_variable || !entry.has_attr(gimli::DW_AT_location) {
                        continue;
                    }
                    let outside_functions = stack[..stack.len() - 1].iter().all(|t| {
                        matches!(
                            *t,
                            gimli::DW_TAG_compile_unit
                                | gimli::DW_TAG_namespace
                                | gimli::DW_TAG_structure_type
                                | gimli::DW_TAG_enumeration_type
                                | gimli::DW_TAG_union_type
                        )
                    });
                    if !outside_functions || !entry.has_attr(gimli::DW_AT_name) {
                        continue;
                    }
                    out.push(GlobalVar {
                        qualified: fmt.qualified_name(entry.offset(), entry),
                        unit: unit_idx,
                        die: entry.offset(),
                    });
                }
            }
            out
        })
    }
}

// ---- evaluation --------------------------------------------------------

enum Val {
    Int(i128),
    Obj(usize, Slot),
}

struct Ctx<'a> {
    table: &'a SymbolTable,
    regs: &'a FrameRegs,
    load_base: u64,
    mem: &'a dyn TargetMemory,
    inline_depth: usize,
    locals: OnceCell<Vec<LocalVar>>,
}

impl Ctx<'_> {
    /// Module-relative pc of the frame.
    fn pc(&self) -> u64 {
        self.regs.pc().wrapping_sub(self.load_base)
    }

    fn fmt(&self, unit: usize) -> Result<Formatter<'_>, String> {
        let info = self
            .table
            .units()
            .get(unit)
            .ok_or_else(|| UNAVAILABLE.to_string())?;
        Ok(Formatter::new(self.table, info, self.mem))
    }

    fn obj(&self, e: &Expr) -> Result<(usize, Slot), String> {
        match self.eval(e)? {
            Val::Obj(unit, slot) => Ok((unit, slot)),
            Val::Int(_) => Err("an integer literal has no fields".into()),
        }
    }

    fn eval(&self, e: &Expr) -> Result<Val, String> {
        Ok(match e {
            Expr::Int(n) => Val::Int(*n),
            Expr::Path(segments) => {
                let (unit, slot) = self.lookup(segments)?;
                Val::Obj(unit, slot)
            }
            Expr::Field(base, name) => {
                let (unit, slot) = self.obj(base)?;
                Val::Obj(unit, self.fmt(unit)?.field(&slot, name)?)
            }
            Expr::Index(base, index) => {
                let (unit, slot) = self.obj(base)?;
                let i = match self.eval(index)? {
                    Val::Int(n) => n,
                    Val::Obj(u, s) => self.fmt(u)?.integer(&s)?,
                };
                Val::Obj(unit, self.fmt(unit)?.index(&slot, i)?)
            }
            Expr::Deref(inner) => {
                let (unit, slot) = match self.eval(inner)? {
                    Val::Obj(unit, slot) => (unit, slot),
                    Val::Int(_) => return Err("an integer cannot be dereferenced".into()),
                };
                Val::Obj(unit, self.fmt(unit)?.deref(&slot)?)
            }
        })
    }

    fn lookup(&self, segments: &[String]) -> Result<(usize, Slot), String> {
        let path = segments.join("::");
        if let [name] = segments {
            let locals = self
                .locals
                .get_or_init(|| self.table.locals_in_frame(self.pc(), self.inline_depth));
            if let Some(var) = locals.iter().find(|v| &v.name == name) {
                return match self
                    .table
                    .locate_local(var, self.regs, self.load_base, self.mem)
                {
                    Ok(slot) => Ok((var.unit, slot)),
                    Err(NOT_IN_SCOPE) => Err(format!("`{name}` is not in scope yet")),
                    Err(OPTIMIZED_OUT) => Err(optimized_out(name)),
                    Err(_) => Err(unavailable(name)),
                };
            }
        }
        self.global(&path)
            .ok_or_else(|| format!("cannot find value `{path}` in this scope"))?
    }

    /// A static named `path` (exactly, or as the tail of its qualified
    /// name), preferring the current frame's compilation unit.
    fn global(&self, path: &str) -> Option<Result<(usize, Slot), String>> {
        let suffix = format!("::{path}");
        let here = self.table.unit_for_pc(self.pc());
        let matches = |g: &&GlobalVar| g.qualified == path || g.qualified.ends_with(&suffix);
        let globals = self.table.globals();
        let found = globals
            .iter()
            .filter(matches)
            .find(|g| Some(g.unit) == here)
            .or_else(|| globals.iter().find(matches))?;
        Some(self.locate_global(found, path))
    }

    fn locate_global(&self, g: &GlobalVar, path: &str) -> Result<(usize, Slot), String> {
        let info = self
            .table
            .units()
            .get(g.unit)
            .ok_or_else(|| UNAVAILABLE.to_string())?;
        let entry = info.unit.entry(g.die).map_err(|e| e.to_string())?;
        let frame = Frame::new(self.regs, self.mem, self.load_base);
        let ty = self
            .table
            .attr_with_origin(info, &entry, gimli::DW_AT_type)
            .and_then(|v| unit_ref(info, v));
        match self.table.locate(info, &entry, g.die, &frame) {
            Located::Place(place) => Ok((
                g.unit,
                Slot {
                    name: path.to_string(),
                    expr: path.to_string(),
                    ty,
                    place,
                },
            )),
            Located::OptimizedOut => Err(optimized_out(path)),
            Located::Unavailable => Err(unavailable(path)),
        }
    }
}

fn optimized_out(name: &str) -> String {
    format!("`{name}` is optimized out")
}

fn unavailable(name: &str) -> String {
    format!("`{name}` is unavailable here")
}

// ---- parsing -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expr {
    Path(Vec<String>),
    Int(i128),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Deref(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Ident(String),
    Int(i128),
    Dot,
    Star,
    LBracket,
    RBracket,
    LParen,
    RParen,
    PathSep,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(tok) = punct(c) {
            out.push(tok);
            i += 1;
            continue;
        }
        match c {
            c if c.is_whitespace() => i += 1,
            ':' if chars.get(i + 1) == Some(&':') => {
                out.push(Tok::PathSep);
                i += 2;
            }
            '&' => return Err("taking a reference (`&`) is not supported".into()),
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().filter(|&&c| c != '_').collect();
                out.push(Tok::Int(parse_int(&word)?));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            other => return Err(format!("unexpected `{other}`")),
        }
    }
    Ok(out)
}

/// The token a single-character punctuator stands for.
fn punct(c: char) -> Option<Tok> {
    Some(match c {
        '.' => Tok::Dot,
        '*' => Tok::Star,
        '[' => Tok::LBracket,
        ']' => Tok::RBracket,
        '(' => Tok::LParen,
        ')' => Tok::RParen,
        _ => return None,
    })
}

/// A Rust integer literal: decimal, `0x`/`0o`/`0b`, optional type suffix.
fn parse_int(word: &str) -> Result<i128, String> {
    const SUFFIXES: [&str; 12] = [
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
    ];
    let (radix, digits) = match word.get(..2) {
        Some("0x") => (16, &word[2..]),
        Some("0o") => (8, &word[2..]),
        Some("0b") => (2, &word[2..]),
        _ => (10, word),
    };
    let digits = SUFFIXES
        .iter()
        .find_map(|s| digits.strip_suffix(s))
        .unwrap_or(digits);
    i128::from_str_radix(digits, radix).map_err(|_| format!("invalid number `{word}`"))
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

fn parse(src: &str) -> Result<Expr, String> {
    if src.is_empty() {
        return Err("empty expression".into());
    }
    let mut p = Parser {
        toks: lex(src)?,
        pos: 0,
    };
    let e = p.unary(0)?;
    match p.peek() {
        None => Ok(e),
        Some(t) => Err(format!("unexpected {}", describe(t))),
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("`{s}`"),
        Tok::Int(n) => format!("`{n}`"),
        Tok::Dot => "`.`".into(),
        Tok::Star => "`*`".into(),
        Tok::LBracket => "`[`".into(),
        Tok::RBracket => "`]`".into(),
        Tok::LParen => "`(`".into(),
        Tok::RParen => "`)`".into(),
        Tok::PathSep => "`::`".into(),
    }
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn expect(&mut self, want: Tok) -> Result<(), String> {
        match self.next() {
            Some(t) if t == want => Ok(()),
            Some(t) => Err(format!(
                "expected {}, found {}",
                describe(&want),
                describe(&t)
            )),
            None => Err(format!("expected {}", describe(&want))),
        }
    }

    fn unary(&mut self, depth: usize) -> Result<Expr, String> {
        if depth > MAX_EXPR_NESTING {
            return Err("expression is nested too deeply".into());
        }
        if self.peek() == Some(&Tok::Star) {
            self.pos += 1;
            return Ok(Expr::Deref(Box::new(self.unary(depth + 1)?)));
        }
        let mut e = self.primary(depth)?;
        loop {
            match self.peek() {
                Some(Tok::Dot) => {
                    self.pos += 1;
                    let field = match self.next() {
                        Some(Tok::Ident(s)) => s,
                        Some(Tok::Int(n)) if n >= 0 => n.to_string(),
                        Some(t) => {
                            return Err(format!("expected a field name, found {}", describe(&t)))
                        }
                        None => return Err("expected a field name".into()),
                    };
                    e = Expr::Field(Box::new(e), field);
                }
                Some(Tok::LBracket) => {
                    self.pos += 1;
                    let index = self.unary(depth + 1)?;
                    self.expect(Tok::RBracket)?;
                    e = Expr::Index(Box::new(e), Box::new(index));
                }
                _ => return Ok(e),
            }
        }
    }

    fn primary(&mut self, depth: usize) -> Result<Expr, String> {
        match self.next() {
            Some(Tok::Int(n)) => Ok(Expr::Int(n)),
            Some(Tok::LParen) => {
                let e = self.unary(depth + 1)?;
                self.expect(Tok::RParen)?;
                Ok(e)
            }
            Some(Tok::Ident(first)) => {
                let mut path = vec![first];
                while self.peek() == Some(&Tok::PathSep) {
                    self.pos += 1;
                    match self.next() {
                        Some(Tok::Ident(s)) => path.push(s),
                        _ => return Err("expected a name after `::`".into()),
                    }
                }
                Ok(Expr::Path(path))
            }
            Some(t) => Err(format!("unexpected {}", describe(&t))),
            None => Err("unexpected end of expression".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> Box<Expr> {
        Box::new(Expr::Path(s.split("::").map(String::from).collect()))
    }

    #[test]
    fn parses_postfix_chains_and_deref() {
        assert_eq!(
            parse("*a.b[3].0").unwrap(),
            Expr::Deref(Box::new(Expr::Field(
                Box::new(Expr::Index(
                    Box::new(Expr::Field(path("a"), "b".into())),
                    Box::new(Expr::Int(3))
                )),
                "0".into()
            )))
        );
        assert_eq!(
            parse("(*p).x").unwrap(),
            Expr::Field(Box::new(Expr::Deref(path("p"))), "x".into())
        );
        assert_eq!(parse("v[i]").unwrap(), Expr::Index(path("v"), path("i")));
        assert_eq!(
            parse("t.0.1").unwrap(),
            Expr::Field(Box::new(Expr::Field(path("t"), "0".into())), "1".into())
        );
        assert_eq!(*path("a::B"), parse("a :: B").unwrap());
        assert_eq!(parse("0x1f_u8").unwrap(), Expr::Int(31));
    }

    #[test]
    fn rejects_what_it_does_not_support() {
        assert!(parse("").is_err());
        assert!(parse("&x").unwrap_err().contains('&'));
        assert!(parse("a +").is_err());
        assert!(parse("a[1").is_err());
        assert!(parse("a.").is_err());
        assert!(parse("x y").is_err());
        assert!(parse(&"(".repeat(200)).is_err());
    }
}
