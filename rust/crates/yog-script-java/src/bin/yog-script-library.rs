//! Writes the System library's own Java (`yog_script_java::library`) under
//! a Java source root: `yog-script-library <java-api/src/main/java>`.

fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("yog-script-library: <java source root>");
        std::process::exit(2);
    };
    let files = match yog_script_java::library() {
        Ok(files) => files,
        Err(problems) => {
            for p in problems {
                eprintln!("yog-script-library: {}", p.message);
            }
            std::process::exit(1);
        }
    };
    for f in files {
        let path = std::path::Path::new(&root).join(&f.path);
        if let Err(e) = yog_codegen::write_if_changed(&path, f.source) {
            eprintln!("yog-script-library: {}: {e}", path.display());
            std::process::exit(1);
        }
    }
}
