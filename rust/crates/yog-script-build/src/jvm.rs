//! Yog Script built for the JVM: each source compiled to Java
//! (`yog-script-java`), `javac` run on them, and a jar made of the classes
//! with its mods listed for `ServiceLoader` — what a `.yog` carries under
//! `scripts/` and the hosts load.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use yog_data::Diagnostic;
use yog_ids::script_problem;

/// Where a jar lists a service's providers.
const SERVICES_DIR: &str = "META-INF/services";
/// What separates a package's names.
const DOT: char = '.';
/// What separates a jar's folders: `/` whatever the platform (zip's).
const SLASH: char = '/';

/// What a build for one game version needs: `javac`, the Java the version
/// runs (its `java_version`), what the code is compiled against (that
/// version's loader jar), the package its classes go in, a folder to work in.
pub struct JvmBuild<'a> {
    pub javac: &'a Path,
    pub release: u32,
    pub classpath: &'a [PathBuf],
    pub package: &'a str,
    pub work: &'a Path,
    /// Its mod says it sends to the server (`yog.toml`'s `[network] to_server`).
    pub to_server: bool,
    /// The mod's files its sources include.
    pub files: Option<&'a dyn yog_script_analyze::macros::ModFiles>,
    /// The mod's id: its content's namespace.
    pub mod_id: &'a str,
    /// The mod's content files, each (kind, id): what its content
    /// declarations bind to.
    pub content: &'a [(String, String)],
}

/// `sources` (name, text) built into the jar `jar`, or what's wrong.
pub fn build_jar(
    sources: &[(String, String)],
    b: &JvmBuild<'_>,
    jar: &Path,
) -> Result<(), Vec<Diagnostic>> {
    let src = b.work.join("java");
    let classes = b.work.join("classes");
    let fail = |e: std::io::Error| {
        vec![Diagnostic::error(
            script_problem::NOT_COMPILED,
            e.to_string(),
        )]
    };
    let _ = std::fs::remove_dir_all(b.work);
    std::fs::create_dir_all(&classes).map_err(fail)?;
    let mut files = Vec::new();
    let mut mods = Vec::new();
    let env = yog_script_analyze::Env {
        to_server: b.to_server,
        files: b.files,
        content: Some(b.content),
        ..Default::default()
    };
    // Every source's declarations first: each source reaches the others'
    // types, as a C# project's files do.
    let decls: Vec<Vec<yog_script_analyze::ast::Decl>> = sources
        .iter()
        .map(|(name, text)| yog_script_analyze::declarations(name, text, env))
        .collect();
    for (i, (name, text)) in sources.iter().enumerate() {
        let others: Vec<&[yog_script_analyze::ast::Decl]> = project_but(&decls, i)
            .map(|(_, d)| d)
            .collect();
        let env = yog_script_analyze::Env {
            others: &others,
            ..env
        };
        let checked = yog_script_analyze::check_in(name, text, env);
        if checked.problems.iter().any(Diagnostic::is_error) {
            return Err(checked.problems);
        }
        let named: Vec<(&str, &[yog_script_analyze::ast::Decl])> = project_but(&decls, i)
            .map(|(j, d)| (sources[j].0.as_str(), d))
            .collect();
        let target = yog_script_java::Target {
            package: b.package,
            mod_id: Some(b.mod_id),
            others: &named,
        };
        let class = yog_script_java::compile_in((name, text), &checked, &target)?;
        // The package's folders, joined as the platform joins them.
        let dir = b
            .package
            .split(DOT)
            .fold(src.clone(), |path, part| path.join(part));
        std::fs::create_dir_all(&dir).map_err(fail)?;
        let file = dir.join(format!("{}.java", class.name));
        std::fs::write(&file, &class.source).map_err(fail)?;
        files.push(file);
        mods.extend(class.mods);
    }
    javac(b, &files, &classes)?;
    write_jar(&classes, &mods, jar).map_err(fail)
}

/// Every source's declarations but the `skip`th's, each with its index.
fn project_but(
    decls: &[Vec<yog_script_analyze::ast::Decl>],
    skip: usize,
) -> impl Iterator<Item = (usize, &[yog_script_analyze::ast::Decl])> {
    decls
        .iter()
        .enumerate()
        .filter(move |(j, _)| *j != skip)
        .map(|(j, d)| (j, d.as_slice()))
}

/// `javac` on `files`, its classes into `classes`.
fn javac(b: &JvmBuild<'_>, files: &[PathBuf], classes: &Path) -> Result<(), Vec<Diagnostic>> {
    let classpath = std::env::join_paths(b.classpath).map_err(|e| {
        vec![Diagnostic::error(
            script_problem::NOT_COMPILED,
            e.to_string(),
        )]
    })?;
    let out = Command::new(b.javac)
        .arg("--release")
        .arg(b.release.to_string())
        .arg("-encoding")
        .arg("UTF-8")
        .arg("-d")
        .arg(classes)
        .arg("-cp")
        .arg(classpath)
        .args(files)
        .output()
        .map_err(|e| {
            vec![Diagnostic::error(
                script_problem::NOT_COMPILED,
                format!("javac: {e}"),
            )]
        })?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    Err(vec![Diagnostic::error(
        script_problem::NOT_COMPILED,
        format!("javac rejected the compiled code (a compiler bug):\n{said}"),
    )])
}

/// The jar: every class under `classes`, and the mods listed.
fn write_jar(classes: &Path, mods: &[String], jar: &Path) -> std::io::Result<()> {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(jar)?);
    let options = zip::write::SimpleFileOptions::default();
    for file in walk(classes)? {
        let name = file
            .strip_prefix(classes)
            .map_err(std::io::Error::other)?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(&SLASH.to_string());
        zip.start_file(name, options)?;
        zip.write_all(&std::fs::read(&file)?)?;
    }
    if !mods.is_empty() {
        zip.start_file(
            format!(
                "{SERVICES_DIR}{SLASH}{}",
                yog_script_ids::jvm::MOD_INTERFACE
            ),
            options,
        )?;
        zip.write_all(mods.join("\n").as_bytes())?;
    }
    zip.finish()?;
    Ok(())
}

/// Every file under `dir`.
fn walk(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            out.extend(walk(&path)?);
        } else {
            out.push(path);
        }
    }
    Ok(out)
}
