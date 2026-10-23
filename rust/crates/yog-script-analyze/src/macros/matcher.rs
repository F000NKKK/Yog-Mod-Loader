//! A call's tokens matched against a rule: what each `$name` took.

use std::collections::HashMap;

use super::fragment::Fragment;
use super::rules::{Matcher, Rep};
use super::tree::Tree;
use yog_analysis::Report;

/// What a fragment took: its trees, or one take per repetition.
#[derive(Clone)]
pub(crate) enum Bound {
    One(Vec<Tree>, Fragment),
    Many(Vec<Bound>),
}

pub(crate) type Bindings = HashMap<String, Bound>;

/// `trees` matched by `ms` whole: what each name took.
pub(crate) fn matched(ms: &[Matcher], trees: &[Tree], report: &Report<'_>) -> Option<Bindings> {
    let mut b = Bindings::new();
    (seq(ms, trees, &mut b, report)? == trees.len()).then_some(b)
}

/// How many of `trees` `ms` take from their start.
fn seq(ms: &[Matcher], trees: &[Tree], b: &mut Bindings, report: &Report<'_>) -> Option<usize> {
    let mut at = 0;
    for m in ms {
        at += one(m, &trees[at..], b, report)?;
    }
    Some(at)
}

fn one(m: &Matcher, trees: &[Tree], b: &mut Bindings, report: &Report<'_>) -> Option<usize> {
    match m {
        Matcher::Token(t) => {
            matches!(trees.first(), Some(Tree::Leaf(l)) if l.tok == *t).then_some(1)
        }
        Matcher::Group { open, inner } => match trees.first() {
            Some(Tree::Group {
                open: o,
                inner: got,
                ..
            }) if o.tok == *open => (seq(inner, got, b, report)? == got.len()).then_some(1),
            _ => None,
        },
        Matcher::Fragment { name, kind } => {
            let n = kind.take(trees, report)?;
            b.insert(name.clone(), Bound::One(trees[..n].to_vec(), *kind));
            Some(n)
        }
        Matcher::Repeat { .. } => repeat(m, trees, b, report),
    }
}

/// `$( inner ) sep op`: as many takes as fit, each name bound per take.
fn repeat(m: &Matcher, trees: &[Tree], b: &mut Bindings, report: &Report<'_>) -> Option<usize> {
    let Matcher::Repeat { inner, sep, op } = m else {
        return None;
    };
    let (sep, op) = (sep.as_ref(), *op);
    let mut takes: Vec<Bindings> = Vec::new();
    let mut at = 0;
    loop {
        let gap = match (takes.is_empty(), sep, trees.get(at)) {
            (true, _, _) | (_, None, _) => 0,
            (false, Some(s), Some(Tree::Leaf(l))) if l.tok == *s => 1,
            _ => break,
        };
        let mut take = Bindings::new();
        match seq(inner, &trees[at + gap..], &mut take, report) {
            Some(n) if n > 0 => {
                takes.push(take);
                at += gap + n;
            }
            _ => break,
        }
        if op == Rep::Maybe {
            break;
        }
    }
    if op == Rep::Some && takes.is_empty() {
        return None;
    }
    for name in names(inner) {
        let all = takes.iter().filter_map(|t| t.get(&name).cloned()).collect();
        b.insert(name, Bound::Many(all));
    }
    Some(at)
}

/// The names a matcher binds.
fn names(ms: &[Matcher]) -> Vec<String> {
    let mut out = Vec::new();
    for m in ms {
        match m {
            Matcher::Fragment { name, .. } => out.push(name.clone()),
            Matcher::Group { inner, .. } | Matcher::Repeat { inner, .. } => {
                out.extend(names(inner))
            }
            Matcher::Token(_) => {}
        }
    }
    out
}
