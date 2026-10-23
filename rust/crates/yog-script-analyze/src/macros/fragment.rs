//! What a `$name:kind` takes: Rust's fragment kinds, each matched by
//! reading the tokens as that form would be.

use super::tree::{Tree, flatten, size};
use crate::parser::Parser;
use crate::source::token::{Tok, Token};
use crate::words;
use yog_analysis::Report;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fragment {
    Expr,
    Ty,
    Ident,
    Pat,
    Stmt,
    Block,
    Literal,
    Tt,
}

/// Each kind by the name a matcher writes after `:`.
const KINDS: &[(&str, Fragment)] = &[
    ("expr", Fragment::Expr),
    ("ty", Fragment::Ty),
    ("ident", Fragment::Ident),
    ("pat", Fragment::Pat),
    ("stmt", Fragment::Stmt),
    ("block", Fragment::Block),
    ("literal", Fragment::Literal),
    ("tt", Fragment::Tt),
];

impl Fragment {
    pub fn named(name: &str) -> Option<Fragment> {
        KINDS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
    }

    /// Every kind's name, for a problem's message.
    pub fn names() -> Vec<&'static str> {
        KINDS.iter().map(|(n, _)| *n).collect()
    }

    /// How many of `trees` this fragment takes from their start, if it
    /// fits there.
    pub fn take(self, trees: &[Tree], report: &Report<'_>) -> Option<usize> {
        let first = trees.first()?;
        match self {
            Fragment::Tt => Some(1),
            Fragment::Ident => {
                matches!(first, Tree::Leaf(t) if matches!(t.tok, Tok::Ident(_))).then_some(1)
            }
            Fragment::Literal => matches!(first, Tree::Leaf(t) if is_literal(&t.tok)).then_some(1),
            _ => self.parsed(trees, report),
        }
    }

    /// Read the trees as this form; the trees it took, when it ends on a
    /// tree's edge.
    fn parsed(self, trees: &[Tree], report: &Report<'_>) -> Option<usize> {
        let mut toks: Vec<Token> = Vec::new();
        flatten(trees, &mut toks);
        let end = toks.last().map(|t| t.span).unwrap_or_default();
        toks.push(Token {
            tok: Tok::Eof,
            span: end,
        });
        let mut scratch = report.scratch();
        let mut p = Parser::new(&toks, &mut scratch, false);
        let ok = match self {
            Fragment::Expr => p.expr().is_ok(),
            Fragment::Ty => p.ty().is_ok(),
            Fragment::Pat => p.pattern().is_ok(),
            Fragment::Stmt => p.stmt().is_ok(),
            Fragment::Block => p.block().is_ok(),
            _ => false,
        };
        let used = p.consumed();
        if !ok || !scratch.out.is_empty() || used == 0 {
            return None;
        }
        let mut count = 0;
        let mut taken = 0;
        for t in trees {
            if count == used {
                break;
            }
            count += size(t);
            taken += 1;
        }
        (count == used).then_some(taken)
    }
}

fn is_literal(t: &Tok) -> bool {
    match t {
        Tok::Int(_)
        | Tok::Float(_)
        | Tok::Unit(..)
        | Tok::Str(_)
        | Tok::RawStr(_)
        | Tok::Char(_) => true,
        Tok::Keyword(w) => **w == words::r#true::WORD || **w == words::r#false::WORD,
        _ => false,
    }
}
