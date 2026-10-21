//! `yog-data-export <file.ts>`: the binary form's constants as TypeScript.

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: yog-data-export <file.ts>");
        std::process::exit(2);
    };
    if let Err(e) = std::fs::write(&path, yog_data::export::typescript()) {
        eprintln!("{path}: {e}");
        std::process::exit(1);
    }
}
