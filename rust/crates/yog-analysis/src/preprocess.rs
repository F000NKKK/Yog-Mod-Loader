//! A preprocessor, as C#'s, for any language with line directives: what
//! `#if`/`#elif`/`#else`/`#endif` leave out of a source blanked (its lines
//! kept, so every place in it stays where it was), `#define`/`#undef`
//! changing the variables. It knows nothing of what it preprocesses:
//! the variables come from outside (`MC = 1.21.1`, `DEBUG`), conditions
//! compare them (`MC >= 1.21 && !DEBUG`).

mod cond;

use std::collections::HashMap;
use std::ops::Range;

/// The variables conditions read: name → value (`"1.21.1"`, `"true"`).
pub type Defines = HashMap<String, String>;

/// What a directive starts with.
const HASH: char = '#';
/// The directives.
const IF: &str = "if";
const ELIF: &str = "elif";
const ELSE: &str = "else";
const ENDIF: &str = "endif";
const DEFINE: &str = "define";
const UNDEF: &str = "undef";
/// What a defined name with no value is.
pub const TRUE: &str = "true";

/// What's wrong with a directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// An `#elif`, `#else` or `#endif` without its `#if`, or one after `#else`.
    Unmatched,
    /// An `#if` never ended.
    Unterminated,
    /// A condition that doesn't read.
    BadCondition(String),
}

/// A source preprocessed: its text with what's left out blanked, whether it
/// had directives at all, and what's wrong (each at its directive's bytes).
#[derive(Debug, Default)]
pub struct Preprocessed {
    pub text: String,
    pub directives: bool,
    pub problems: Vec<(Range<usize>, Problem)>,
}

/// An `#if` being read: whether the code around it is in, whether a branch
/// of it was taken, whether the current one is, whether `#else` was met.
struct Frame {
    outer: bool,
    taken: bool,
    active: bool,
    ended: bool,
    at: Range<usize>,
}

/// The directives' state while a source is read.
struct Run {
    defines: Defines,
    frames: Vec<Frame>,
    problems: Vec<(Range<usize>, Problem)>,
}

/// `src` with `defines`: what its directives leave in.
#[must_use]
pub fn preprocess(src: &str, defines: &Defines) -> Preprocessed {
    let mut run = Run {
        defines: defines.clone(),
        frames: Vec::new(),
        problems: Vec::new(),
    };
    let mut out = Preprocessed {
        text: String::with_capacity(src.len()),
        ..Preprocessed::default()
    };
    let mut at = 0;
    for line in src.split_inclusive('\n') {
        let span = at..at + line.trim_end().len();
        at += line.len();
        let Some((word, rest)) = directive(line) else {
            let keep = run.active();
            out.text
                .push_str(&if keep { line.to_string() } else { blank(line) });
            continue;
        };
        out.directives = true;
        out.text.push_str(&blank(line));
        run.directive(word, rest, span);
    }
    if let Some(f) = run.frames.last() {
        run.problems.push((f.at.clone(), Problem::Unterminated));
    }
    out.problems = run.problems;
    out
}

impl Run {
    /// Whether the code here is in.
    fn active(&self) -> bool {
        self.frames.last().is_none_or(|f| f.active)
    }

    fn directive(&mut self, word: &str, rest: &str, span: Range<usize>) {
        let active = self.active();
        match word {
            DEFINE | UNDEF if active => {
                let name = rest.trim().to_string();
                if word == DEFINE {
                    self.defines.insert(name, TRUE.into());
                } else {
                    self.defines.remove(&name);
                }
            }
            DEFINE | UNDEF => {}
            IF => {
                let held = active && self.holds(rest, &span);
                self.frames.push(Frame {
                    outer: active,
                    taken: held,
                    active: held,
                    ended: false,
                    at: span,
                });
            }
            _ => self.branch(word, rest, span),
        }
    }

    /// `#elif`, `#else`, `#endif`: the frame they're in moved on.
    fn branch(&mut self, word: &str, rest: &str, span: Range<usize>) {
        let Some(ended) = self.frames.last().map(|f| f.ended) else {
            self.problems.push((span, Problem::Unmatched));
            return;
        };
        if ended && word != ENDIF {
            self.problems.push((span, Problem::Unmatched));
            return;
        }
        let held = word == ELIF && self.holds(rest, &span);
        let Some(f) = self.frames.last_mut() else {
            return;
        };
        match word {
            ELIF => {
                f.active = f.outer && !f.taken && held;
                f.taken |= f.active;
            }
            ELSE => {
                f.active = f.outer && !f.taken;
                f.taken = true;
                f.ended = true;
            }
            _ => {
                self.frames.pop();
            }
        }
    }

    /// Whether condition `text` holds; a broken one reported (and false).
    fn holds(&mut self, text: &str, span: &Range<usize>) -> bool {
        cond::eval(text, &self.defines).unwrap_or_else(|e| {
            self.problems.push((span.clone(), Problem::BadCondition(e)));
            false
        })
    }
}

/// A line's directive: its word and the rest of the line, when its first
/// non-blank is `#` and a directive's word.
fn directive(line: &str) -> Option<(&'static str, &str)> {
    let rest = line.trim_start().strip_prefix(HASH)?;
    let len = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    let word = [IF, ELIF, ELSE, ENDIF, DEFINE, UNDEF]
        .into_iter()
        .find(|w| *w == &rest[..len])?;
    let after = rest[len..].split("//").next().unwrap_or_default();
    Some((word, after))
}

/// A line left out: its characters spaces, its line break kept.
fn blank(line: &str) -> String {
    line.chars()
        .map(|c| if c == '\n' || c == '\r' { c } else { ' ' })
        .collect()
}

#[cfg(test)]
mod tests;
