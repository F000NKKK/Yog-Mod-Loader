//! Which of a command's handlers its arguments fit, and how: several
//! handlers share a name (subcommands of different shapes), tried in
//! [`Fit::ORDER`] until one answers.

/// What separates a typed command's arguments in [`crate::CommandContext::args`].
/// What joins the words a greedy argument takes.
const WORD_SEP: &str = " ";
/// The argument type that takes the rest of the line.
pub const GREEDY: &str = "string";

/// How a handler takes a command's arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// A typed handler with exactly as many arguments as were given.
    Exact,
    /// A typed handler ending in a greedy `string`, the extra words joined
    /// into it (the game may have parsed them as separate words).
    Joined,
    /// A plain (schema-less) handler, with whatever was typed.
    Plain,
}

impl Fit {
    /// The order handlers are tried in.
    pub const ORDER: [Fit; 3] = [Fit::Exact, Fit::Joined, Fit::Plain];

    /// The ways a handler with `schema` (`None`: plain) takes arguments.
    pub fn ways(schema: Option<&str>) -> &'static [Fit] {
        match schema {
            Some(_) => &Self::ORDER[..2],
            None => &Self::ORDER[2..],
        }
    }

    /// Where it's tried: earlier ways rank higher.
    pub fn rank(self) -> usize {
        Self::ORDER.len() - Self::ORDER.iter().position(|f| *f == self).unwrap_or(Self::ORDER.len())
    }

    /// `args` as a handler with `schema` takes them this way, or `None` if
    /// they don't fit it.
    pub fn take(self, schema: Option<&str>, args: &[String]) -> Option<Vec<String>> {
        match (self, schema) {
            (Fit::Exact, Some(schema)) => (schema.split_whitespace().count() == args.len()).then(|| args.to_vec()),
            (Fit::Joined, Some(schema)) => {
                let types: Vec<&str> = schema.split_whitespace().collect();
                let k = types.len();
                if types.last() != Some(&GREEDY) || args.len() <= k {
                    return None;
                }
                let mut parts = args[..k - 1].to_vec();
                parts.push(args[k - 1..].join(WORD_SEP));
                Some(parts)
            }
            (Fit::Plain, None) => Some(args.to_vec()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn exact_takes_as_many_arguments_as_the_schema_has() {
        assert_eq!(Fit::Exact.take(Some("int word"), &v(&["1", "x"])), Some(v(&["1", "x"])));
        assert_eq!(Fit::Exact.take(Some("int word"), &v(&["1"])), None);
        assert_eq!(Fit::Exact.take(Some(""), &[]), Some(Vec::new()));
    }

    #[test]
    fn joined_folds_the_extra_words_into_a_last_greedy_string() {
        assert_eq!(Fit::Joined.take(Some("int string"), &v(&["1", "a", "b"])), Some(v(&["1", "a b"])));
        assert_eq!(Fit::Joined.take(Some("int word"), &v(&["1", "a", "b"])), None);
        assert_eq!(Fit::Joined.take(Some("int string"), &v(&["1", "a"])), None, "an exact fit isn't a join");
    }

    #[test]
    fn plain_takes_anything_and_only_plain_handlers() {
        assert_eq!(Fit::Plain.take(None, &v(&["a", "b"])), Some(v(&["a", "b"])));
        assert_eq!(Fit::Plain.take(Some("int"), &v(&["1"])), None);
        assert_eq!(Fit::Exact.take(None, &[]), None);
    }

    #[test]
    fn earlier_ways_rank_higher() {
        assert!(Fit::Exact.rank() > Fit::Joined.rank() && Fit::Joined.rank() > Fit::Plain.rank());
        assert_eq!(Fit::ways(None), [Fit::Plain]);
    }
}
