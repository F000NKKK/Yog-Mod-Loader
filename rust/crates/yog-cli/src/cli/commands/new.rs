//! `yog new <name> [--rust]`: a new mod, in Yog Script unless `--rust`.

pub(super) const WORDS: &[&str] = &["new"];

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    let rust = rest
        .iter()
        .any(|a| a == crate::cli::project::new::RUST_FLAG);
    let name = rest
        .iter()
        .find(|a| *a != crate::cli::project::new::RUST_FLAG)
        .map(String::as_str);
    let language = if rust {
        yog_formats::manifest::Language::Rust
    } else {
        yog_formats::manifest::Language::Ys
    };
    crate::cli::project::new::new_mod(name, language)
}
