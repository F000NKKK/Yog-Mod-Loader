//! External compilers inherit the host's environment, outside an AppImage.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use yog_script_build::{JvmBuild, build_jar};

const PROBE: &str = "YOG_TEST_JAVAC_ENVIRONMENT";

#[test]
fn javac_uses_host_environment() {
    if let Some(directory) = std::env::var_os(PROBE) {
        build_in_probe(Path::new(&directory));
        return;
    }
    let directory = std::env::temp_dir().join(format!("yog-javac-env-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let javac = directory.join("javac");
    std::fs::write(
        &javac,
        "#!/bin/sh\n\
         test -z \"${APPDIR+x}\" || exit 1\n\
         test -z \"${APPIMAGE+x}\" || exit 2\n\
         test \"$LD_LIBRARY_PATH\" = /usr/lib || exit 3\n",
    )
    .unwrap();
    std::fs::set_permissions(&javac, std::fs::Permissions::from_mode(0o755)).unwrap();
    let appdir = directory.join("appimage");

    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "javac_uses_host_environment", "--nocapture"])
        .env(PROBE, &directory)
        .env("APPDIR", &appdir)
        .env("APPIMAGE", directory.join("installer.AppImage"))
        .env(
            "LD_LIBRARY_PATH",
            std::env::join_paths([appdir.join("lib"), "/usr/lib".into()]).unwrap(),
        )
        .status()
        .unwrap();

    assert!(status.success(), "javac inherited the AppImage environment");
    assert!(directory.join("compiled.jar").is_file());
    std::fs::remove_dir_all(directory).unwrap();
}

fn build_in_probe(directory: &Path) {
    let javac = directory.join("javac");
    let work = directory.join("work");
    let build = JvmBuild {
        javac: &javac,
        release: 21,
        classpath: &[],
        package: "test",
        work: &work,
        to_server: false,
        files: None,
        mod_id: "test",
        content: &[],
    };

    build_jar(&[], &build, &directory.join("compiled.jar")).unwrap();
}
