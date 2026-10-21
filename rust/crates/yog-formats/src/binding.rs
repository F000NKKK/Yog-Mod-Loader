//! Bindings in a `.yui` attribute's value: `{x:Bind name}` stands for the
//! value of that name (a book page's field, a recipe's value).

use yog_ids::yui_markup::{BIND_CLOSE, BIND_OPEN};

/// A piece of an attribute's value: words as written, or a binding's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Piece<'a> {
    Text(&'a str),
    Bind(&'a str),
}

/// `attr`'s pieces in order; a binding never closed is words.
pub fn pieces(attr: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = attr;
    while let Some(i) = rest.find(BIND_OPEN) {
        let after = &rest[i + BIND_OPEN.len()..];
        let Some(end) = after.find(BIND_CLOSE) else {
            break;
        };
        if i > 0 {
            out.push(Piece::Text(&rest[..i]));
        }
        out.push(Piece::Bind(after[..end].trim()));
        rest = &after[end + BIND_CLOSE.len()..];
    }
    if !rest.is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

/// The names `attr` binds.
pub fn names(attr: &str) -> impl Iterator<Item = &str> {
    pieces(attr).into_iter().filter_map(|p| match p {
        Piece::Bind(name) => Some(name),
        Piece::Text(_) => None,
    })
}

/// The name `attr` binds when it's one binding and nothing else.
pub fn whole(attr: &str) -> Option<&str> {
    let name = attr
        .trim()
        .strip_prefix(BIND_OPEN)?
        .strip_suffix(BIND_CLOSE)?;
    (!name.contains(BIND_CLOSE)).then(|| name.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_and_whole_bindings() {
        assert_eq!(
            pieces("{u|{x:Bind input}} → {x:Bind  output }"),
            [
                Piece::Text("{u|"),
                Piece::Bind("input"),
                Piece::Text("} → "),
                Piece::Bind("output")
            ]
        );
        assert_eq!(pieces("{x:Bind open"), [Piece::Text("{x:Bind open")]);
        assert_eq!(whole(" {x:Bind patterns} "), Some("patterns"));
        assert_eq!(whole("{x:Bind a}{x:Bind b}"), None);
    }
}
