//! `include_str!("help.txt")`: a file of the mod read when the source is,
//! as Rust's — the call replaced by the file's text, a string written as
//! it is (no `{…}` read in it). The build reads the file, beside the
//! source; without one (an editor's check) the call stays, typed a string.

use yog_analysis::Report;
use yog_ids::script_problem;

use super::builtin::INCLUDE_STR;
use crate::source::token::{Punct, StrPart, Tok, Token};

/// What reads a mod's files for its sources.
pub trait ModFiles {
    /// File `path`, written in source `source` (beside it): its text, or
    /// why it can't be read.
    fn read(&self, source: &str, path: &str) -> Result<String, String>;
}

/// Each `include_str!("path")` in `tokens` (of source `source`) replaced by
/// the file's text; one that can't be read reported, left as it is.
pub(crate) fn expand(
    mut tokens: Vec<Token>,
    (source, files): (&str, &dyn ModFiles),
    report: &mut Report<'_>,
) -> Vec<Token> {
    let mut i = 0;
    while i < tokens.len() {
        let Some(path) = call_at(&tokens, i) else {
            i += 1;
            continue;
        };
        let at = tokens[i].span.to(tokens[i + CALL_LEN - 1].span);
        match files.read(source, &path) {
            Ok(text) => {
                let lit = Token {
                    tok: Tok::RawStr(text),
                    span: at,
                };
                tokens.splice(i..i + CALL_LEN, [lit]);
            }
            Err(why) => {
                report.error(
                    script_problem::MACRO,
                    at,
                    format!("`{}!` can't read `{path}`: {why}", INCLUDE_STR.name),
                );
                i += CALL_LEN;
            }
        }
        i += 1;
    }
    tokens
}

/// `include_str ! ( "path" )`: its tokens.
const CALL_LEN: usize = 5;

/// The path of the `include_str!` call at `i`, its argument a plain string.
fn call_at(tokens: &[Token], i: usize) -> Option<String> {
    let call = tokens.get(i..i + CALL_LEN)?;
    let named = matches!(&call[0].tok, Tok::Ident(n) if n == INCLUDE_STR.name);
    let shaped = call[1].tok == Tok::Punct(Punct::Bang)
        && call[2].tok == Tok::Punct(Punct::LParen)
        && call[4].tok == Tok::Punct(Punct::RParen);
    if !named || !shaped {
        return None;
    }
    match &call[3].tok {
        Tok::RawStr(s) => Some(s.clone()),
        Tok::Str(parts) => match parts.as_slice() {
            [StrPart::Text(s)] => Some(s.clone()),
            [] => Some(String::new()),
            _ => None,
        },
        _ => None,
    }
}
