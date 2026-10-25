//! The System library's own Java: what the compiler writes of it is what
//! the runtime has (`yog-script-library` writes it again).

#[test]
fn the_librarys_java_is_current() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../java-api/src/main/java");
    let files = yog_script_java::library().unwrap_or_else(|p| panic!("{p:#?}"));
    assert!(!files.is_empty());
    for f in files {
        let on_disk = std::fs::read_to_string(root.join(&f.path)).unwrap_or_default();
        assert!(
            on_disk == f.source,
            "{} is stale: cargo run -p yog-script-java --bin yog-script-library -- {}",
            f.path,
            root.display()
        );
    }
}
