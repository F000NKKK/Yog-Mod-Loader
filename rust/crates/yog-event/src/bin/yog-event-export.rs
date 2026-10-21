//! Writes the bus's messages for Java: `yog-event-export <dir> <package>` →
//! `<dir>/YogEvents.java` and `<dir>/YogServices.java`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, package] = args.as_slice() else {
        eprintln!("yog-event-export: <dir> <package>");
        std::process::exit(2);
    };
    for (class, text) in yog_event::java::files(package) {
        let path = format!("{dir}/{class}.java");
        if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        if let Err(e) = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&path, text)) {
            eprintln!("yog-event-export: {path}: {e}");
            std::process::exit(1);
        }
    }
}
