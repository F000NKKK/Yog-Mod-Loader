//! Writes the bus's messages for Java: `yog-event-export <api-dir> <host-dir> <package>`
//! → `<api-dir>/YogEvents.java`, `YogServices.java`, `YogTypes.java`,
//! `YogMirrors.java` (the API's: `java-api`) and `<host-dir>/YogHostedServices.java`
//! (the host's: `java-common`).

use yog_event::java::Side;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [api, host, package] = args.as_slice() else {
        eprintln!("yog-event-export: <api-dir> <host-dir> <package>");
        std::process::exit(2);
    };
    for (side, class, text) in yog_event::java::files(package) {
        let dir = match side {
            Side::Api => api,
            Side::Host => host,
        };
        let path = format!("{dir}/{class}.java");
        if let Err(e) = yog_codegen::write_if_changed(std::path::Path::new(&path), text) {
            eprintln!("yog-event-export: {path}: {e}");
            std::process::exit(1);
        }
    }
}
