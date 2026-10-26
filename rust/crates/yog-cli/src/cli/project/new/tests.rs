use super::*;

#[test]
fn a_new_mod_starts_in_its_language() {
    assert_eq!(spelled(Language::Ys), "ys");
    assert_eq!(spelled(Language::Rust), "rust");
    assert_eq!(first_source(Language::Ys).0, "src/main.ys");
    assert_eq!(first_source(Language::Rust).0, "src/lib.rs");
    // What `yog new` writes reads back as the language it was made in.
    let toml = YOG_TOML
        .replace("$NAME$", "demo")
        .replace("$DISPLAY$", "Demo")
        .replace("$LANGUAGE$", &spelled(Language::Rust));
    let file = std::env::temp_dir().join(format!("yog-new-{}.toml", std::process::id()));
    std::fs::write(&file, toml).unwrap();
    let meta = yog_cli::project::manifest::YogToml::read(&file);
    std::fs::remove_file(&file).unwrap();
    assert_eq!(meta.unwrap().language, Language::Rust);
}
