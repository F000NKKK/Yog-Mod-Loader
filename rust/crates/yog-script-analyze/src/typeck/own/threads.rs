//! How a type's members cross threads, read off its declarations: its
//! stored and static fields, its `[Crosses(…)]` parameters, its
//! `[Threads(…)]` (`typeck/threads`).

use yog_script_ids::system::threads::{CROSSES, THREAD_STATIC, THREADS};

use super::OwnType;
use crate::ast::{Attribute, Decl, ExprKind, Modifier, StrSeg};

/// Field (or auto-property) `name` declared by `d`: stored, or static.
pub(super) fn field(t: &mut OwnType, d: &Decl, name: &str) {
    if d.has(Modifier::Const) {
        return;
    }
    if !d.has(Modifier::Static) {
        t.stored.push(name.to_string());
        return;
    }
    t.statics.push(name.to_string());
    if attributes(d, THREAD_STATIC).next().is_some() {
        t.thread_statics.push(name.to_string());
    }
}

/// `d`'s `[Threads(ThreadSafety.…)]`: its case.
pub(super) fn threads(d: &Decl) -> Option<String> {
    let a = attributes(d, THREADS).next()?;
    case(&a.args.first()?.value.kind)
}

/// `d`'s `[Crosses(nameof(work), Crossing.…)]`: each parameter and its crossing.
pub(super) fn crossings(d: &Decl) -> Vec<(String, String)> {
    attributes(d, CROSSES)
        .filter_map(|a| match a.args.as_slice() {
            [param, crossing] => Some((text(&param.value.kind)?, case(&crossing.value.kind)?)),
            _ => None,
        })
        .collect()
}

fn attributes<'d>(d: &'d Decl, name: &'d str) -> impl Iterator<Item = &'d Attribute> {
    d.attributes
        .iter()
        .filter(move |a| a.path.last().is_some_and(|n| n.text == name))
}

/// `Enum.Case`'s case.
fn case(e: &ExprKind) -> Option<String> {
    match e {
        ExprKind::Member { name, .. } => Some(name.text.clone()),
        _ => None,
    }
}

/// A plain string's text, or `nameof(x)`'s.
fn text(e: &ExprKind) -> Option<String> {
    match e {
        ExprKind::NameOf(of) => match &of.kind {
            ExprKind::Ident(name) => Some(name.clone()),
            _ => None,
        },
        ExprKind::Str(segs) => match segs.as_slice() {
            [StrSeg::Text(t)] => Some(t.clone()),
            _ => None,
        },
        _ => None,
    }
}
