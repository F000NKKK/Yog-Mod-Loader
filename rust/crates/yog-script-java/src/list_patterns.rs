//! List patterns in Java: `[1, let b, .., let z]` on `v` is its length
//! checked (`Ys.lengthOf(v) >= 3`), then each item's pattern on its item —
//! counted from the start before the `..`, from the end after it — and
//! `.. let rest` on `Ys.slice(v, …)`, of `v`'s own kind.

use yog_script_analyze::Type;
use yog_script_analyze::ast::Pattern;
use yog_script_analyze::token::Span;
use yog_script_ids::java::{IF, NULL};
use yog_script_ids::jvm::{HELPERS, LENGTH_OF, RUNTIME, SLICE};

use super::Gen;

/// An item of a list pattern: its pattern, where it's read, its type.
type Read<'p> = (&'p Pattern, String, Option<Type>);

impl Gen<'_> {
    /// `[items]` on `v` (of `on`) as a condition.
    pub(crate) fn list_test(
        &mut self,
        items: &[Pattern],
        v: &str,
        on: Option<&Type>,
        at: Span,
    ) -> Option<String> {
        let (length, reads) = self.list_reads(items, (v, on), at)?;
        let mut all = vec![length];
        for (p, read, t) in reads {
            all.push(self.test(p, &read, t.as_ref())?);
        }
        Some(all.join(" && "))
    }

    /// `[items]` on `v` (of `on`) opened as `if`s, its names bound inside:
    /// how many to close.
    pub(crate) fn list_open(&mut self, items: &[Pattern], (v, on): (&str, Option<&Type>), at: Span) -> usize {
        let Some((length, reads)) = self.list_reads(items, (v, on), at) else {
            return 0;
        };
        self.code.open(format!("{IF} ({length})"));
        1 + reads
            .into_iter()
            .map(|(p, read, t)| self.pattern_open(p, (&read, t.as_ref())))
            .sum::<usize>()
    }

    /// The length `[items]` needs of `v` as a condition, and each item's
    /// pattern with where it's read.
    fn list_reads<'p>(
        &mut self,
        items: &'p [Pattern],
        (v, on): (&str, Option<&Type>),
        at: Span,
    ) -> Option<(String, Vec<Read<'p>>)> {
        let Some(on) = on else {
            self.not_yet("a list pattern on a value of no known type", at);
            return None;
        };
        let item = yog_script_analyze::list_item(on);
        let slice = items.iter().position(|p| matches!(p, Pattern::Slice(_)));
        let fixed = items.len() - usize::from(slice.is_some());
        let len = format!("{RUNTIME}.{HELPERS}.{LENGTH_OF}({v})");
        let cmp = if slice.is_some() { ">=" } else { "==" };
        let length = format!("{v} != {NULL} && {len} {cmp} {fixed}");
        let before = slice.unwrap_or(items.len());
        let after = items.len() - before - usize::from(slice.is_some());
        let mut reads = Vec::new();
        for (i, p) in items.iter().enumerate() {
            let read = match p {
                Pattern::Slice(None) => continue,
                Pattern::Slice(Some(rest)) => {
                    let kind = self.java(on.non_null(), false);
                    let to = format!("{len} - {after}");
                    let read = format!("(({kind}) {RUNTIME}.{HELPERS}.{SLICE}({v}, {before}, {to}, false))");
                    reads.push((&**rest, read, Some(on.non_null().clone())));
                    continue;
                }
                _ if i < before => i.to_string(),
                _ => format!("({len} - {})", items.len() - i),
            };
            reads.push((p, self.item_read(v, on, &read)?, item.clone()));
        }
        Some((length, reads))
    }

    /// Item `i` of `v` (of `on`, an array, a string or a list).
    fn item_read(&mut self, v: &str, on: &Type, i: &str) -> Option<String> {
        Some(match on.non_null() {
            Type::Array(..) => format!("{v}[{i}]"),
            Type::String => {
                let host = self.index.host(&Type::String)?;
                Self::index_read(&host, &format!("{v}, {i}"))
            }
            _ => Self::index_read(v, i),
        })
    }
}
