//! The commands `yog-script` takes, each in its own file.

mod check;
mod index;
mod macros;
mod source;
mod test;

pub(super) const USAGE: &str = "usage: yog-script check [--macros <file>]... <file>... | yog-script test [--macros <file>]... <file>... | yog-script index <minecraft version> | yog-script source <path>";

type Run = fn(&[String]) -> Result<(), String>;

pub(super) struct Command {
    words: &'static [&'static str],
    pub(super) run: Run,
}

const ALL: &[Command] = &[
    Command {
        words: check::WORDS,
        run: check::run,
    },
    Command {
        words: index::WORDS,
        run: index::run,
    },
    Command {
        words: source::WORDS,
        run: source::run,
    },
    Command {
        words: test::WORDS,
        run: test::run,
    },
];

pub(super) fn find(word: Option<&str>) -> Option<&'static Command> {
    let word = word?;
    ALL.iter().find(|c| c.words.contains(&word))
}
