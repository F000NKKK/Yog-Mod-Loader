//! Writes the names for the other languages:
//! `yog-ids-export --ts <file.ts> --java <dir> <package> <Class>`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--ts" if i + 1 < args.len() => {
                write(&args[i + 1], &yog_ids::export::typescript());
                i += 2;
            }
            "--java" if i + 3 < args.len() => {
                let (dir, package, class) = (&args[i + 1], &args[i + 2], &args[i + 3]);
                write(
                    &format!("{dir}/{class}.java"),
                    &yog_ids::export::java(package, class),
                );
                i += 4;
            }
            other => {
                eprintln!(
                    "yog-ids-export: unexpected `{other}` — --ts <file.ts> | --java <dir> <package> <Class>"
                );
                std::process::exit(2);
            }
        }
    }
}

/// Writes `text` to `path` when it differs (so builds don't see a change).
fn write(path: &str, text: &str) {
    if let Err(e) = yog_codegen::write_if_changed(std::path::Path::new(path), text) {
        eprintln!("yog-ids-export: {path}: {e}");
        std::process::exit(1);
    }
}
