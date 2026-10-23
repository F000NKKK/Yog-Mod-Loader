//! Tokens as trees: a bracketed group is one tree, as macros see code.

use crate::source::token::{Punct, Span, Tok, Token};

#[derive(Clone)]
pub(crate) enum Tree {
    Leaf(Token),
    /// `( … )`, `[ … ]`, `{ … }`: its brackets and what's inside.
    Group {
        open: Token,
        inner: Vec<Tree>,
        close: Token,
    },
}

/// The bracket that closes `open`, if it opens a group.
pub(crate) fn closer(open: &Tok) -> Option<Punct> {
    match open {
        Tok::Punct(Punct::LParen) => Some(Punct::RParen),
        Tok::Punct(Punct::LBracket) => Some(Punct::RBracket),
        Tok::Punct(Punct::LBrace) => Some(Punct::RBrace),
        _ => None,
    }
}

/// The tree starting at `i`, and where the next one starts.
pub(crate) fn tree_at(tokens: &[Token], i: usize) -> (Tree, usize) {
    let open = tokens[i].clone();
    let Some(close) = closer(&open.tok) else {
        return (Tree::Leaf(open), i + 1);
    };
    let mut inner = Vec::new();
    let mut j = i + 1;
    while j < tokens.len() {
        if tokens[j].tok == Tok::Punct(close) {
            let close = tokens[j].clone();
            return (Tree::Group { open, inner, close }, j + 1);
        }
        let (tree, next) = tree_at(tokens, j);
        inner.push(tree);
        j = next;
    }
    let close = Token {
        tok: Tok::Punct(close),
        span: Span::new(open.span.end as usize, open.span.end as usize),
    };
    (Tree::Group { open, inner, close }, j)
}

/// Trees back to tokens.
pub(crate) fn flatten(trees: &[Tree], out: &mut Vec<Token>) {
    for t in trees {
        match t {
            Tree::Leaf(tok) => out.push(tok.clone()),
            Tree::Group { open, inner, close } => {
                out.push(open.clone());
                flatten(inner, out);
                out.push(close.clone());
            }
        }
    }
}

/// How many tokens a tree is.
pub(crate) fn size(t: &Tree) -> usize {
    match t {
        Tree::Leaf(_) => 1,
        Tree::Group { inner, .. } => 2 + inner.iter().map(size).sum::<usize>(),
    }
}
