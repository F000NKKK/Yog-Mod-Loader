//! Writes the bus's messages for Java: `yog-event-export <dir> <package>` →
//! `<dir>/YogEvents.java`, `YogServices.java`, `YogTypes.java` and `YogMirrors.java`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, package] = args.as_slice() else {
        eprintln!("yog-event-export: <dir> <package>");
        std::process::exit(2);
    };
    for (class, text) in yog_event::java::files(package) {
        let path = format!("{dir}/{class}.java");
        if let Err(e) = yog_codegen::write_if_changed(std::path::Path::new(&path), text) {
            eprintln!("yog-event-export: {path}: {e}");
            std::process::exit(1);
        }
    }
}
