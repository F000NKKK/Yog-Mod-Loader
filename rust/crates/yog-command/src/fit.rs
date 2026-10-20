//! Which of a command's handlers its arguments fit, and how: several
//! handlers share a name (subcommands of different shapes), tried in
//! [`Fit::ORDER`] until one answers.

/// What separates a typed command's arguments in [`crate::CommandContext::args`].
pub const ARG_SEP: char = '\t';
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
    pub fn take(self, schema: Option<&str>, args: &str) -> Option<String> {
        let given: Vec<&str> = if args.is_empty() { Vec::new() } else { args.split(ARG_SEP).collect() };
        match (self, schema) {
            (Fit::Exact, Some(schema)) => (schema.split_whitespace().count() == given.len()).then(|| args.to_owned()),
            (Fit::Joined, Some(schema)) => {
                let types: Vec<&str> = schema.split_whitespace().collect();
                let k = types.len();
                if types.last() != Some(&GREEDY) || given.len() <= k {
                    return None;
                }
                let mut parts: Vec<String> = given[..k - 1].iter().map(|a| (*a).to_owned()).collect();
                parts.push(given[k - 1..].join(" "));
                Some(parts.join(&ARG_SEP.to_string()))
            }
            (Fit::Plain, None) => Some(args.to_owned()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_takes_as_many_arguments_as_the_schema_has() {
        assert_eq!(Fit::Exact.take(Some("int word"), "1\tx"), Some("1\tx".into()));
        assert_eq!(Fit::Exact.take(Some("int word"), "1"), None);
        assert_eq!(Fit::Exact.take(Some(""), ""), Some(String::new()));
    }

    #[test]
    fn joined_folds_the_extra_words_into_a_last_greedy_string() {
        assert_eq!(Fit::Joined.take(Some("int string"), "1\ta\tb"), Some("1\ta b".into()));
        assert_eq!(Fit::Joined.take(Some("int word"), "1\ta\tb"), None);
        assert_eq!(Fit::Joined.take(Some("int string"), "1\ta"), None, "an exact fit isn't a join");
    }

    #[test]
    fn plain_takes_anything_and_only_plain_handlers() {
        assert_eq!(Fit::Plain.take(None, "a\tb"), Some("a\tb".into()));
        assert_eq!(Fit::Plain.take(Some("int"), "1"), None);
        assert_eq!(Fit::Exact.take(None, ""), None);
    }

    #[test]
    fn earlier_ways_rank_higher() {
        assert!(Fit::Exact.rank() > Fit::Joined.rank() && Fit::Joined.rank() > Fit::Plain.rank());
        assert_eq!(Fit::ways(None), [Fit::Plain]);
    }
}
