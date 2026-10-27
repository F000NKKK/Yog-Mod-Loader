//! Yog Script run on a real JVM: sources checked as one project, compiled
//! to Java, built by `javac` with the runtime (`java-api`'s jar), and a
//! static method's value printed by `java`. Skipped, saying so, where
//! there's no JDK or no runtime jar (`./build.sh build api`).

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_script_analyze::ast::Decl;
use yog_script_analyze::{Env, check_in, declarations};

/// The JDK the tests run with: `YOG_TEST_JDK`, else Java 21's.
const JDK: &str = "/usr/lib/jvm/java-21-openjdk-amd64";
/// The package the sources compile into.
const PACKAGE: &str = "t";

/// The runtime's jar, built by `./build.sh build api`.
/// The variable that keeps a run's Java and classes.
const KEEP: &str = "YOG_KEEP_JVM";

fn runtime_jar() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../java-api/build/libs/yog-java-api.jar")
}

fn jdk() -> PathBuf {
    std::env::var_os("YOG_TEST_JDK").map_or_else(|| PathBuf::from(JDK), PathBuf::from)
}

/// What `expr` (Java, over the compiled classes: `t.MainScript.Run()`)
/// prints once `sources` (name, text) are built; `None` when there's no
/// JVM to run on. A source that doesn't check or compile panics.
pub fn run(sources: &[(&str, &str)], expr: &str) -> Option<String> {
    run_warned(sources, expr, &[])
}

/// `run`, the sources warned of each of `warned` (codes) and of nothing else.
pub fn run_warned(sources: &[(&str, &str)], expr: &str, warned: &[&str]) -> Option<String> {
    let (jdk, jar) = (jdk(), runtime_jar());
    if !jdk.join("bin/javac").is_file() || !jar.is_file() {
        eprintln!(
            "SKIPPED: no JDK at {} or no {}",
            jdk.display(),
            jar.display()
        );
        return None;
    }
    // One folder a run: tests run side by side.
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let work = std::env::temp_dir().join(format!("yog-script-jvm-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let src = work.join("src").join(PACKAGE);
    std::fs::create_dir_all(&src).unwrap();
    let decls: Vec<Vec<Decl>> = sources
        .iter()
        .map(|(n, t)| declarations(n, t, Env::default()))
        .collect();
    let mut files = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (i, (name, text)) in sources.iter().enumerate() {
        let others: Vec<&[Decl]> = decls
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, d)| d.as_slice())
            .collect();
        let env = Env {
            others: &others,
            ..Env::default()
        };
        let checked = check_in(name, text, env);
        let code = |p: &yog_data::Diagnostic| {
            p.code
                .map(yog_data::Code::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let unexpected = checked
            .problems
            .iter()
            .filter(|p| p.is_error() || !warned.contains(&code(p).as_str()));
        assert!(unexpected.count() == 0, "{name}: {:#?}", checked.problems);
        seen.extend(checked.problems.iter().map(code));
        let named: Vec<(&str, &[Decl])> = sources
            .iter()
            .zip(&decls)
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, ((n, _), d))| (*n, d.as_slice()))
            .collect();
        let target = yog_script_java::Target {
            package: PACKAGE,
            mod_id: Some("test"),
            others: &named,
        };
        let class = yog_script_java::compile_in((name, text), &checked, &target)
            .unwrap_or_else(|p| panic!("{name}: {p:#?}"));
        let file = src.join(format!("{}.java", class.name));
        std::fs::write(&file, class.source).unwrap();
        files.push(file);
    }
    for code in warned {
        assert!(seen.iter().any(|c| c == code), "no {code} warning");
    }
    let main = work.join("src").join("Main.java");
    std::fs::write(
        &main,
        format!("public class Main {{ public static void main(String[] a) {{ System.out.println({expr}); }} }}\n"),
    )
    .unwrap();
    files.push(main);
    let classes = work.join("classes");
    let javac = Command::new(jdk.join("bin/javac"))
        .arg("-nowarn")
        .arg("-cp")
        .arg(&jar)
        .arg("-d")
        .arg(&classes)
        .args(&files)
        .output()
        .unwrap();
    assert!(
        javac.status.success(),
        "javac: {}",
        String::from_utf8_lossy(&javac.stderr)
    );
    let cp = std::env::join_paths([classes.as_path(), jar.as_path()]).unwrap();
    let java = Command::new(jdk.join("bin/java"))
        .arg("-cp")
        .arg(cp)
        .arg("Main")
        .output()
        .unwrap();
    assert!(
        java.status.success(),
        "java: {}",
        String::from_utf8_lossy(&java.stderr)
    );
    // `YOG_KEEP_JVM=1` keeps the Java written, to read when a test fails.
    if std::env::var_os(KEEP).is_none() {
        let _ = std::fs::remove_dir_all(&work);
    }
    Some(String::from_utf8_lossy(&java.stdout).trim_end().to_owned())
}
