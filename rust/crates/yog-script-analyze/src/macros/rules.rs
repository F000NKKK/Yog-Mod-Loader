//! A `macro` declaration read into its rules: each a matcher (literal
//! tokens, `$name:kind` fragments, `$( … ) sep op` repetitions) and the
//! body it writes.

use yog_ids::script_problem;

use super::fragment::Fragment;
use super::tree::Tree;
use crate::source::token::{Punct, Span, Tok};
use yog_analysis::Report;

pub(crate) struct Macro {
    pub rules: Vec<Rule>,
}

pub(crate) struct Rule {
    pub matcher: Vec<Matcher>,
    pub body: Vec<Tree>,
}

pub(crate) enum Matcher {
    Token(Tok),
    Group {
        open: Tok,
        inner: Vec<Matcher>,
    },
    Fragment {
        name: String,
        kind: Fragment,
    },
    Repeat {
        inner: Vec<Matcher>,
        sep: Option<Tok>,
        op: Rep,
    },
}

/// How often a repetition matches: `*`, `+`, `?`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rep {
    Any,
    Some,
    Maybe,
}

impl Rep {
    fn of(t: &Tok) -> Option<Rep> {
        match t {
            Tok::Punct(Punct::Star) => Some(Rep::Any),
            Tok::Punct(Punct::Plus) => Some(Rep::Some),
            Tok::Punct(Punct::Question) => Some(Rep::Maybe),
            _ => None,
        }
    }
}

/// `{ (matcher) => { body }; … }`'s rules.
pub(crate) fn read(body: &[Tree], at: Span, report: &mut Report<'_>) -> Macro {
    let mut rules = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let rule = match (&body[i], body.get(i + 1), body.get(i + 2)) {
            (
                Tree::Group { inner: m, .. },
                Some(Tree::Leaf(arrow)),
                Some(Tree::Group { inner: b, .. }),
            ) if arrow.tok == Tok::Punct(Punct::FatArrow) => Rule {
                matcher: matchers(m, report),
                body: b.clone(),
            },
            _ => {
                report.error(
                    script_problem::MACRO,
                    at,
                    "a macro's rule is `(matcher) => { body };`",
                );
                break;
            }
        };
        rules.push(rule);
        i += 3;
        if matches!(body.get(i), Some(Tree::Leaf(t)) if t.tok == Tok::Punct(Punct::Semi)) {
            i += 1;
        }
    }
    Macro { rules }
}

fn matchers(trees: &[Tree], report: &mut Report<'_>) -> Vec<Matcher> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < trees.len() {
        match &trees[i] {
            Tree::Leaf(t) if t.tok == Tok::Punct(Punct::Dollar) => {
                let (m, next) = dollar(trees, i + 1, t.span, report);
                out.extend(m);
                i = next;
                continue;
            }
            Tree::Leaf(t) => out.push(Matcher::Token(t.tok.clone())),
            Tree::Group { open, inner, .. } => out.push(Matcher::Group {
                open: open.tok.clone(),
                inner: matchers(inner, report),
            }),
        }
        i += 1;
    }
    out
}

/// After a `$`: `name:kind` or `( … ) sep? op`; where reading goes on.
fn dollar(trees: &[Tree], i: usize, at: Span, report: &mut Report<'_>) -> (Option<Matcher>, usize) {
    match trees.get(i) {
        Some(Tree::Leaf(t)) => {
            let Tok::Ident(name) = &t.tok else {
                return (None, i + 1);
            };
            let kind = match (trees.get(i + 1), trees.get(i + 2)) {
                (Some(Tree::Leaf(c)), Some(Tree::Leaf(k))) if c.tok == Tok::Punct(Punct::Colon) => {
                    match &k.tok {
                        Tok::Ident(k) => Fragment::named(k),
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(kind) = kind else {
                let kinds = Fragment::names().join(", ");
                report.error(
                    script_problem::MACRO,
                    at,
                    format!("`${name}` needs a kind: `${name}:expr` ({kinds})"),
                );
                return (None, i + 1);
            };
            (
                Some(Matcher::Fragment {
                    name: name.clone(),
                    kind,
                }),
                i + 3,
            )
        }
        Some(Tree::Group { inner, .. }) => {
            let inner = matchers(inner, report);
            let leaf = |j: usize| match trees.get(j) {
                Some(Tree::Leaf(t)) => Some(t.tok.clone()),
                _ => None,
            };
            if let Some(op) = leaf(i + 1).as_ref().and_then(Rep::of) {
                return (
                    Some(Matcher::Repeat {
                        inner,
                        sep: None,
                        op,
                    }),
                    i + 2,
                );
            }
            match (leaf(i + 1), leaf(i + 2).as_ref().and_then(Rep::of)) {
                (Some(sep), Some(op)) => (
                    Some(Matcher::Repeat {
                        inner,
                        sep: Some(sep),
                        op,
                    }),
                    i + 3,
                ),
                _ => {
                    report.error(
                        script_problem::MACRO,
                        at,
                        "a repetition ends with `*`, `+` or `?`: `$($x:expr),*`",
                    );
                    (None, i + 1)
                }
            }
        }
        None => (None, i),
    }
}
