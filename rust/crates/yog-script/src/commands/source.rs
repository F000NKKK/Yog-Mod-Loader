//! `yog-script source <path>`: one source checked as it's being edited —
//! its text on stdin, what's wrong with it on stdout as Yog Data frames (a
//! `Diagnostic` each). What the Yog CLI's service asks for an editor.

use std::io::Read;

pub(super) const WORDS: &[&str] = &["source"];
const USAGE: &str = "usage: yog-script source <path>  (its text on stdin)";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let [path] = rest else {
        return Err(USAGE.into());
    };
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let mut out = std::io::stdout().lock();
    for d in yog_script_build::check_source(path, &text) {
        yog_data::io::write_frame(&mut out, &d).map_err(|e| e.to_string())?;
    }
    Ok(())
}
