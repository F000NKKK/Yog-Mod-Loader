//! `yog-formats-export <file.ts>`: the models' vocabulary as TypeScript.

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: yog-formats-export <file.ts>");
        std::process::exit(2);
    };
    if let Err(e) = yog_codegen::write_if_changed(path.as_ref(), yog_formats::export::typescript())
    {
        eprintln!("{path}: {e}");
        std::process::exit(1);
    }
}
