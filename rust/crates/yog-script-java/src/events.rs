//! `event`s, as C#'s: a type's (or the file's) handlers of a delegate type,
//! in the runtime's `Event` — `+=` adds one, `-=` removes it (a method by
//! its object and itself, so `-= Handler` takes back `+= Handler`), raising
//! it calls each. An `[Event("…")]` one is on the bus: `+=` listens to the
//! address, raising it emits there.

use std::fmt::Write as _;

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Arg, BinOp, Decl, Expr, ExprKind, Modifier};
use yog_script_ids::clr::INVOKE;
use yog_script_ids::java::{CLASS_LITERAL, FINAL, FOR, INT, NEW, NULL, RETURN, VAR};
use yog_script_ids::jvm::{
    BUS_EMIT, BUS_ON, BUS_PRIORITY, EVENT_ADD, EVENT_CLASS, EVENT_HANDLERS, EVENT_REMOVE, KEY_OF,
    LIST_GET, LIST_SIZE, OVERLOAD_SEP, RUNNABLE, SUPPLIER,
};
use yog_script_ids::system::EVENT;

use crate::decls::Place;
use crate::{Gen, modifier};

/// An event code names: its handlers' `Event` in Java, its address when
/// it's on the bus, its delegate's type.
pub(crate) struct EventRef {
    holder: String,
    address: Option<String>,
    delegate: Type,
}

impl Gen<'_> {
    /// Event `d` of delegate type `ty` (Java's), in `place`: its handlers'
    /// `Event`; an `[Event]` one's none — the bus holds them.
    pub(crate) fn event_decl(&mut self, d: &Decl, (ty, name): (&str, &str), place: Place) {
        if crate::index::attribute_text(d, EVENT).is_some() {
            return;
        }
        let stat = if place == Place::File || d.has(Modifier::Static) {
            modifier(yog_script_ids::java::STATIC)
        } else {
            String::new()
        };
        let access = if d.has(Modifier::Private) {
            modifier(yog_script_ids::java::PRIVATE)
        } else {
            modifier(yog_script_ids::java::PUBLIC)
        };
        self.code.line(format!(
            "{access}{stat}{FINAL} {EVENT_CLASS}<{ty}> {} = {NEW} {EVENT_CLASS}<>();",
            Self::ident(name)
        ));
    }

    /// The event `e` names, when it names one.
    pub(crate) fn event_of(&mut self, e: &Expr) -> Option<EventRef> {
        let delegate = self.types.of(e).cloned()?;
        let (owner, name) = match &e.kind {
            ExprKind::Ident(name) if self.field_of(name).is_none() => {
                (self.owner.as_ref().map(|(o, _)| o.clone()), name.clone())
            }
            ExprKind::Member { expr, name, .. } => match self.types.of(expr).map(Type::non_null) {
                Some(Type::Own(o, _) | Type::Static(o, _)) => (Some(o.clone()), name.text.clone()),
                _ => return None,
            },
            _ => return None,
        };
        let address = self
            .index
            .event(owner.as_deref(), &name)
            .or_else(|| self.index.event(None, &name))?
            .map(|a| self.code.str(a));
        let holder = match &e.kind {
            ExprKind::Member { expr, .. } => match self.types.of(expr) {
                Some(Type::Static(o, args)) => {
                    format!("{}.{}", self.index.place(o, args.len()), Self::ident(&name))
                }
                _ => format!("{}.{}", self.expr(expr), Self::ident(&name)),
            },
            _ => Self::ident(&name),
        };
        Some(EventRef {
            holder,
            address,
            delegate,
        })
    }

    /// `event += handler` or `event -= handler`.
    pub(crate) fn subscribe(&mut self, ev: &EventRef, op: BinOp, handler: &Expr) {
        let d = self.expr(handler);
        match (&ev.address, op) {
            (Some(address), BinOp::Add) => {
                let (payload, each) = (self.payload(ev), self.temp());
                let made = self.temp();
                let t = self.java(&ev.delegate, false);
                self.code.line(format!("{FINAL} {t} {made} = {d};"));
                self.code.line(format!(
                    "{BUS_ON}({address}, {BUS_PRIORITY}, {payload}, ({each}) -> {{ {made}.{INVOKE}({each}); {RETURN} {NULL}; }});"
                ));
            }
            (Some(_), _) => self.not_yet("`-=` on an event on the bus", handler.span),
            (None, op) => {
                let key = self.group_key(handler);
                let call = match (op, key) {
                    (BinOp::Add, Some(k)) => format!("{EVENT_ADD}({k}, {d})"),
                    (BinOp::Add, None) => format!("{EVENT_ADD}({d})"),
                    (_, Some(k)) => format!("{EVENT_REMOVE}({k})"),
                    (_, None) => format!("{EVENT_REMOVE}({d})"),
                };
                self.code.line(format!("{}.{call};", ev.holder));
            }
        }
    }

    /// Event `ev` raised with `args`: each handler called (giving the
    /// last's value, as C#'s) by the file's raiser of its signature; a bus
    /// one's emitted.
    pub(crate) fn raise(&mut self, e: &Expr, ev: &EventRef, args: &[Arg]) -> String {
        if let Some(address) = &ev.address {
            let a = self.args(args);
            return format!("{BUS_EMIT}({address}, {}, {a})", self.payload(ev));
        }
        let gives = self
            .types
            .of(e)
            .filter(|t| **t != Type::Void)
            .cloned();
        if let Some(raiser) = self.raiser(&ev.delegate, args, gives.as_ref()) {
            let a = self.args(args);
            let sep = if a.is_empty() { "" } else { ", " };
            return format!("{raiser}({}{sep}{a})", ev.holder);
        }
        self.raised_inline(ev, args, gives.as_ref())
    }

    /// Event `ev` raised where no raiser can be written (a type parameter
    /// in its signature): a loop over its handlers in a lambda called at
    /// once, each argument computed first, once.
    fn raised_inline(&mut self, ev: &EventRef, args: &[Arg], gives: Option<&Type>) -> String {
        let mut take = String::new();
        let mut names = Vec::new();
        for a in args {
            let v = self.args(std::slice::from_ref(a));
            let name = self.temp();
            let _ = write!(take, "{VAR} {name} = {v}; ");
            names.push(name);
        }
        let (hs, i, n) = (self.temp(), self.temp(), self.temp());
        let _ = write!(take, "{VAR} {hs} = {}.{EVENT_HANDLERS}();", ev.holder);
        let call = format!("{hs}.{LIST_GET}({i}).{INVOKE}({})", names.join(", "));
        let each = format!("{FOR} ({INT} {i} = 0, {n} = {hs}.{LIST_SIZE}(); {i} < {n}; {i}++)");
        match gives {
            Some(t) => {
                let (t, out) = (self.java(t, true), self.temp());
                format!(
                    "(({SUPPLIER}<{t}>) () -> {{ {take} {t} {out} = {NULL}; {each} {out} = {call}; {RETURN} {out}; }}).get()"
                )
            }
            None => format!("(({RUNNABLE}) () -> {{ {take} {each} {call}; }}).run()"),
        }
    }

    /// What a bus event carries, as a Java class literal: its delegate's
    /// one parameter's.
    fn payload(&self, ev: &EventRef) -> String {
        let t = match &ev.delegate {
            Type::Fn(params, _) => params.first().cloned(),
            Type::Own(_, args) => args.first().cloned(),
            _ => None,
        }
        .unwrap_or(Type::Unknown);
        let erased = crate::reified::erased(&self.java(&t, true));
        format!("{erased}{CLASS_LITERAL}")
    }

    /// What removes method group `e`'s handler: its object (or class) and
    /// the method; none for what isn't a group (a lambda is its own key).
    fn group_key(&mut self, e: &Expr) -> Option<String> {
        let used = self.types.method(e)?.clone();
        if !matches!(e.kind, ExprKind::Ident(_) | ExprKind::Member { .. }) {
            return None;
        }
        let target = match &e.kind {
            ExprKind::Member { expr, .. } => match self.types.of(expr) {
                Some(Type::Static(o, args)) => {
                    format!("{}{CLASS_LITERAL}", self.index.place(o, args.len()))
                }
                _ => self.expr(expr),
            },
            _ if self.statik => format!("{}{CLASS_LITERAL}", self.class_name.clone()),
            _ => self.this_ref(),
        };
        let method = self
            .code
            .str(&format!("{}{OVERLOAD_SEP}{}", used.name, used.overload));
        Some(format!("{KEY_OF}({target}, {method})"))
    }
}
