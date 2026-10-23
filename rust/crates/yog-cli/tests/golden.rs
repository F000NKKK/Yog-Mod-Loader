//! Golden tests: every mod under `tests/golden/<case>/` goes through what
//! `yog build` does with its resources, content and screens (`.yres` →
//! content → outputs merged → `.yui`), and everything that comes out — generated code, files,
//! warnings, errors — must match `tests/golden/<case>.expected`.
//!
//! After a deliberate change: `YOG_BLESS=1 cargo test -p yog-cli --test golden`
//! rewrites the expected files; review their diff.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use yog_cli::ui::yui;
use yog_cli::{content, yres};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

/// Everything the build makes of the mod at `root`, as text.
fn build(root: &Path, mod_id: &str) -> String {
    let mut out = String::new();
    let res = match yres::compile(root, mod_id) {
        Ok(r) => r,
        Err(e) => return format!("# resources failed\n{e}\n"),
    };
    let c = match content::compile(root, mod_id, &res.refs) {
        Ok(c) => c,
        Err(e) => return format!("# content failed\n{e}\n"),
    };
    for w in res.warnings.iter().chain(&c.warnings) {
        let _ = writeln!(out, "# warning: {w}");
    }
    for (set, src) in &res.sources {
        let _ = writeln!(out, "# resources: {set}.g.rs\n{src}");
    }
    if let Some(src) = &c.source {
        let _ = writeln!(out, "# content.g.rs\n{}", src.root);
        for (name, code) in &src.parts {
            let _ = writeln!(out, "# content/{name}.g.rs\n{code}");
        }
    }
    // Data the code includes (books), shown as Yog Data text.
    for (path, bytes) in &c.data_files {
        let shown = yog_data::binary::decode(bytes)
            .map(|v| yog_data::text::to_string_pretty(&v))
            .unwrap_or_else(|e| format!("<not Yog Data: {e}>"));
        let _ = writeln!(out, "# generated/{path}\n{shown}");
    }
    let mut outputs = res.outputs;
    if let Err(e) = content::merge_outputs(root, &mut outputs, c) {
        let _ = writeln!(out, "# merging failed\n{e}");
    }
    yui::set_resource_refs(&res.refs);
    match yui::generate_all(root) {
        Ok(g) if g.screens == 0 => {}
        Ok(_) => {
            let dir = root.join(yui::GENERATED_DIR);
            let mut files = yog_analysis::files::all(&dir);
            files.sort();
            for f in files {
                let rel = f
                    .strip_prefix(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let _ = writeln!(
                    out,
                    "# screens: {rel}\n{}",
                    std::fs::read_to_string(&f).unwrap()
                );
            }
        }
        Err(e) => {
            let _ = writeln!(out, "# screens failed\n{e}");
        }
    }
    outputs.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, bytes) in outputs {
        let shown = match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(v) => serde_json::to_string_pretty(&v).unwrap(),
            Err(_) => format!("<{} bytes>", bytes.len()),
        };
        let _ = writeln!(out, "# {path}\n{shown}");
    }
    out
}

#[test]
fn every_case_builds_as_expected() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let bless = std::env::var_os("YOG_BLESS").is_some();
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    assert!(!cases.is_empty());
    let mut failed = Vec::new();
    for case in cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        // Built in a copy: the build may write next to the sources.
        let root = std::env::temp_dir().join(format!("yog-golden-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(&case, &root);
        let got = build(&root, "gm").replace(&*root.to_string_lossy(), "<mod>");
        let _ = std::fs::remove_dir_all(&root);
        let expected_path = dir.join(format!("{name}.expected"));
        if bless {
            std::fs::write(&expected_path, &got).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
        if got != expected {
            let first = got
                .lines()
                .zip(expected.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            failed.push(format!(
                "{name}: differs from {name}.expected at line {} (got {:?}, expected {:?})",
                first + 1,
                got.lines().nth(first).unwrap_or(""),
                expected.lines().nth(first).unwrap_or("")
            ));
        }
    }
    assert!(
        failed.is_empty(),
        "{}\n(YOG_BLESS=1 rewrites the expected files)",
        failed.join("\n")
    );
}
