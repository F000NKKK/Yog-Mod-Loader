//! Every command `yog` takes, each in its own file: the words that name
//! it and what it does with the rest of the line.

mod add;
mod bench;
mod build;
mod check;
mod content;
mod control;
mod data;
mod design_host;
mod designer_lib;
mod doctor;
mod help;
mod import;
mod mappings;
mod new;
mod publish;
mod remove;
mod res;
mod run;
mod schema;
mod serve;
mod setup;
mod ui;
mod version;

/// What a command's module gives: its words, and its run on the rest of the line.
type Run = fn(&[String]) -> Result<(), String>;

pub(super) struct Command {
    words: &'static [&'static str],
    pub(super) run: Run,
}

macro_rules! commands {
    ($($m:ident),* $(,)?) => {
        const ALL: &[Command] = &[$(Command { words: $m::WORDS, run: $m::run }),*];
    };
}

commands!(
    build,
    check,
    schema,
    bench,
    serve,
    designer_lib,
    design_host,
    new,
    setup,
    doctor,
    version,
    add,
    remove,
    run,
    control,
    data,
    mappings,
    publish,
    res,
    content,
    import,
    ui,
    help,
);

/// The command `word` names; none given: help.
pub(super) fn find(word: Option<&str>) -> Option<&'static Command> {
    let word = word.unwrap_or(help::WORDS[0]);
    ALL.iter().find(|c| c.words.contains(&word))
}
