//! The `yog` command line: each command in its own module
//! ([`commands`]), picked by its first word; what they do in [`pipeline`],
//! [`project`] and [`run`].

mod commands;
mod flags;
mod pipeline;
mod project;
mod run;

/// Runs the command `args` (the program's own name first) names; an error
/// ends the program.
pub(crate) fn main(args: &[String]) {
    let rest = args.get(2..).unwrap_or_default();
    let word = args.get(1).map(String::as_str);
    let result = match commands::find(word) {
        Some(command) => (command.run)(rest),
        None => Err(format!("unknown command: {}", word.unwrap_or_default())),
    };
    if let Err(e) = result {
        // Said already as what it is, or said here as a failure.
        if yog_cli::diagnostics::errors_reported() == 0 {
            yog_cli::diagnostics::failed(yog_ids::build_problem::FAILED, &e);
        }
        eprintln!("yog: error: {e}");
        std::process::exit(1);
    }
}
