//! A mod's Yog Script built for the JVM: a jar per game version the
//! loader SDK runs, made for that version's Java, each in the `.yog` as
//! `scripts/<mc>/<mod>.jar` — the hosts load their own version's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use yog_ids::Format;
use yog_script_build::{JvmBuild, build_jar};

use crate::diagnostics;
use crate::project::manifest::{SOURCES_DIR, YogToml};

/// What a mod's compiled Yog Script's package starts with: `yogscript.<mod>`.
const PACKAGE_ROOT: &str = "yogscript";
/// Where its builds work, under the build folder.
const WORK_DIR: &str = "scripts";
/// The compiler, on `PATH` when no JDK of the version's Java is installed
/// (a newer one compiles for an older release); in a JDK, with the
/// platform's executable suffix (`javac.exe` on Windows).
const JAVAC: &str = "javac";
/// Where a JDK keeps its tools.
const JDK_BIN: &str = "bin";

/// The mod's scripts' jars as `.yog` entries, their content declarations
/// bound to `content` (each file's (kind, id)); none when it has no `.ys`.
pub fn jars(
    root: &Path,
    meta: &YogToml,
    build_dir: &Path,
    content: &[(String, String)],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let sources = sources(root)?;
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let home = yog_home::Home::locate()?;
    let resolver = yog_home::targets::Resolver::new(&home)?;
    let api = resolver.java_api_jar().ok_or(
        "Yog Script needs the loader SDK's java-api jar: `./build.sh build api` in the SDK",
    )?;
    let files = yog_script_build::FilesBeside {
        base: root.to_path_buf(),
    };
    let package = format!("{PACKAGE_ROOT}.{}", yog_ids::mod_lib_name(&meta.id));
    let mut out = Vec::new();
    for (mc, java) in versions(&resolver)? {
        let javac = resolver.jdk(java).map_or_else(
            || PathBuf::from(JAVAC),
            |jdk| {
                jdk.join(JDK_BIN)
                    .join(format!("{JAVAC}{}", std::env::consts::EXE_SUFFIX))
            },
        );
        let work = build_dir.join(WORK_DIR).join(&mc);
        let jar = build_dir
            .join(WORK_DIR)
            .join(format!("{mc}-{}.jar", meta.id));
        let b = JvmBuild {
            javac: &javac,
            release: java,
            classpath: std::slice::from_ref(&api),
            package: &package,
            work: &work,
            to_server: meta.to_server,
            files: Some(&files),
            mod_id: &meta.id,
            content,
        };
        if let Err(problems) = build_jar(&sources, &b, &jar) {
            diagnostics::report_all(&problems);
            problems.iter().for_each(|d| yog_logging::error!("{d}"));
            return Err(format!("the Yog Script didn't build for {mc}"));
        }
        let bytes = std::fs::read(&jar).map_err(|e| format!("{}: {e}", jar.display()))?;
        out.push((
            format!("{}/{mc}/{}.jar", yog_ids::archive::SCRIPTS, meta.id),
            bytes,
        ));
        yog_logging::info!("scripts for {mc} (Java {java})");
    }
    Ok(out)
}

/// The mod's `.ys` sources (name, text), those the build generates
/// (`obj/generated/**`: the content's ids, the resources) among them. A
/// `.yns` (nanites) isn't built for the JVM this way.
pub(crate) fn sources(root: &Path) -> Result<Vec<(String, String)>, String> {
    let generated = crate::files::of(&root.join(yog_ids::project::GENERATED), Format::Script);
    crate::files::of(&root.join(SOURCES_DIR), Format::Script)
        .into_iter()
        .chain(generated)
        .map(|f| {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            Ok((crate::shown_path(root, &f), text))
        })
        .collect()
}

/// The game versions the SDK's loaders run, each with its Java.
fn versions(resolver: &yog_home::targets::Resolver) -> Result<BTreeMap<String, u32>, String> {
    let mut out = BTreeMap::new();
    for t in resolver.targets()? {
        if let (Some(mc), Some(java)) = (t.variant.clone(), t.java()) {
            out.insert(mc, java);
        }
    }
    if out.is_empty() {
        return Err("no game version to build the Yog Script for: no loader SDK".into());
    }
    Ok(out)
}
