//! A rule's body written out with what the call's `$name`s took:
//! repetitions once per take, an expression kept whole in brackets, every
//! token placed at the call (a problem in it points there), the locals the
//! body declares renamed apart from the call's (hygiene).

use std::collections::HashSet;

use yog_ids::script_problem;

use super::fragment::Fragment;
use super::matcher::{Bindings, Bound};
use super::tree::{Tree, flatten};
use crate::source::token::{Punct, Span, Tok, Token};
use crate::words::{r#let, r#mut};
use yog_analysis::Report;

/// What a local a macro declares is renamed to: its name, this, the
/// expansion's number — never a name a source can write.
pub(crate) const HYGIENE_MARK: char = '$';

pub(crate) struct Writer<'r, 's> {
    pub at: Span,
    pub report: &'r mut Report<'s>,
    /// The tokens written, each with whether the body (not the call) gave it.
    out: Vec<(Token, bool)>,
}

impl<'r, 's> Writer<'r, 's> {
    pub fn new(at: Span, report: &'r mut Report<'s>) -> Self {
        Writer {
            at,
            report,
            out: Vec::new(),
        }
    }

    pub fn write(&mut self, body: &[Tree], b: &Bindings) {
        let mut i = 0;
        while i < body.len() {
            i = self.tree(body, i, b);
        }
    }

    /// The tree at `i` written; where the next one starts.
    fn tree(&mut self, body: &[Tree], i: usize, b: &Bindings) -> usize {
        match &body[i] {
            Tree::Leaf(t) if t.tok == Tok::Punct(Punct::Dollar) => self.dollar(body, i + 1, b),
            Tree::Leaf(t) => {
                self.body_token(&t.tok);
                i + 1
            }
            Tree::Group { open, inner, close } => {
                self.body_token(&open.tok);
                self.write(inner, b);
                self.body_token(&close.tok);
                i + 1
            }
        }
    }

    fn body_token(&mut self, tok: &Tok) {
        self.out.push((
            Token {
                tok: tok.clone(),
                span: self.at,
            },
            true,
        ));
    }

    /// After a `$`: a name's take, or a repetition.
    fn dollar(&mut self, body: &[Tree], i: usize, b: &Bindings) -> usize {
        match body.get(i) {
            Some(Tree::Leaf(Token {
                tok: Tok::Ident(name),
                ..
            })) => {
                match b.get(name) {
                    Some(Bound::One(trees, kind)) => self.take(trees, *kind),
                    _ => self.problem(format!(
                        "`${name}` isn't bound here (in a repetition, write it inside `$( … )*`)"
                    )),
                }
                i + 1
            }
            Some(Tree::Group { inner, .. }) => self.repeat(body, i, inner, b),
            _ => i,
        }
    }

    /// A fragment's take, as the call wrote it; an expression in brackets.
    fn take(&mut self, trees: &[Tree], kind: Fragment) {
        let mut toks = Vec::new();
        flatten(trees, &mut toks);
        let wrap = kind == Fragment::Expr && trees.len() > 1;
        if wrap {
            self.body_token(&Tok::Punct(Punct::LParen));
        }
        self.out.extend(toks.into_iter().map(|t| (t, false)));
        if wrap {
            self.body_token(&Tok::Punct(Punct::RParen));
        }
    }

    /// `$( inner ) sep? op`: `inner` once per take of the names in it.
    fn repeat(&mut self, body: &[Tree], i: usize, inner: &[Tree], b: &Bindings) -> usize {
        let leaf = |j: usize| match body.get(j) {
            Some(Tree::Leaf(t)) => Some(t.tok.clone()),
            _ => None,
        };
        let is_op = |t: &Option<Tok>| {
            matches!(
                t,
                Some(Tok::Punct(Punct::Star | Punct::Plus | Punct::Question))
            )
        };
        let (sep, next) = if is_op(&leaf(i + 1)) {
            (None, i + 2)
        } else {
            (leaf(i + 1), i + 3)
        };
        let counts: HashSet<usize> = used(inner)
            .iter()
            .filter_map(|n| match b.get(n) {
                Some(Bound::Many(all)) => Some(all.len()),
                _ => None,
            })
            .collect();
        let [count] = counts.into_iter().collect::<Vec<_>>()[..] else {
            self.problem("a repetition repeats names taken the same number of times".to_string());
            return next;
        };
        for k in 0..count {
            if k > 0
                && let Some(s) = &sep
            {
                self.body_token(s);
            }
            let mut each = b.clone();
            for (name, bound) in b {
                if let Bound::Many(all) = bound {
                    each.insert(name.clone(), all[k].clone());
                }
            }
            self.write(inner, &each);
        }
        next
    }

    fn problem(&mut self, message: String) {
        self.report.error(script_problem::MACRO, self.at, message);
    }

    /// The tokens, the body's locals renamed with expansion `n`'s mark.
    pub fn finish(self, n: usize) -> Vec<Token> {
        let declared = self.declared();
        self.out
            .into_iter()
            .map(|(mut t, from_body)| {
                if let Tok::Ident(name) = &t.tok
                    && from_body
                    && declared.contains(name)
                {
                    t.tok = Tok::Ident(format!("{name}{HYGIENE_MARK}{n}"));
                }
                t
            })
            .collect()
    }
}

impl Writer<'_, '_> {
    /// The names the body's own `let`s and `let mut`s declare (not a name
    /// the call gave: that one is the caller's on purpose).
    fn declared(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for (i, (t, from_body)) in self.out.iter().enumerate() {
            if !from_body || !is_word(&t.tok, &r#let::WORD) {
                continue;
            }
            let mut at = i + 1;
            if self
                .out
                .get(at)
                .is_some_and(|(m, _)| is_word(&m.tok, &r#mut::WORD))
            {
                at += 1;
            }
            if let Some((
                Token {
                    tok: Tok::Ident(name),
                    ..
                },
                true,
            )) = self.out.get(at)
            {
                out.insert(name.clone());
            }
        }
        out
    }
}

fn is_word(t: &Tok, w: &crate::words::Word) -> bool {
    matches!(t, Tok::Keyword(k) if **k == *w)
}

/// The `$name`s a body part writes.
fn used(trees: &[Tree]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, t) in trees.iter().enumerate() {
        match t {
            Tree::Leaf(d) if d.tok == Tok::Punct(Punct::Dollar) => {
                if let Some(Tree::Leaf(Token {
                    tok: Tok::Ident(n), ..
                })) = trees.get(i + 1)
                {
                    out.push(n.clone());
                }
            }
            Tree::Group { inner, .. } => out.extend(used(inner)),
            Tree::Leaf(_) => {}
        }
    }
    out
}
