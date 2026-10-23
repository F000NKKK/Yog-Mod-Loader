//! `yog-script`: Yog Script's tool on its compiler (`yog-script-build`):
//! `yog-script check <file>...` checks sources, `yog-script source <path>`
//! checks one from stdin (for an editor), `yog-script index <mc>` builds a
//! version's class index.

mod commands;

fn main() {
    // Warnings and errors go to stderr, said as the Yog CLI says them.
    yog_logging::set_sink(|level, message| {
        let said = match level {
            yog_logging::Level::Info => "note",
            yog_logging::Level::Warn => "warning",
            yog_logging::Level::Error => "error",
        };
        eprintln!("{said}: {message}");
    });
    let args: Vec<String> = std::env::args().collect();
    let rest = args.get(2..).unwrap_or_default();
    let word = args.get(1).map(String::as_str);
    let result = match commands::find(word) {
        Some(command) => (command.run)(rest),
        None => Err(format!(
            "unknown command: {}\n{}",
            word.unwrap_or_default(),
            commands::USAGE
        )),
    };
    if let Err(e) = result {
        eprintln!("{}: error: {e}", yog_ids::program::SCRIPT);
        std::process::exit(1);
    }
}
