//! Every Java method the runtime calls exists, with the arguments it's called
//! with, in every host's `dev.yog.*` classes (a missing one only shows up as
//! a silent `false` / `None` at run time).
//!
//! Only runs inside the repository: the published crate has no Java sources.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const HOSTS: [&str; 3] = ["fabric", "forge", "neoforge"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn runtime_sources() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = String::new();
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "rs") {
            out.push_str(&fs::read_to_string(p).unwrap());
        }
    }
    out
}

/// The string literal `s` starts with (after whitespace), and what follows it.
fn literal(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start().strip_prefix('"')?;
    let end = s.find('"')?;
    Some((&s[..end], &s[end + 1..]))
}

/// Top-level items of the bracketed list `s` starts with (`[a, f(b, c)]` → 2).
fn list_len(s: &str) -> Option<usize> {
    let s = s.trim_start().strip_prefix('[')?;
    let (mut depth, mut items, mut any) = (0i32, 0usize, false);
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            ']' if depth == 0 => return Some(items + any as usize),
            ']' => depth -= 1,
            // A trailing comma doesn't start another item.
            ',' if depth == 0 => {
                items += any as usize;
                any = false;
                continue;
            }
            _ => {}
        }
        if !c.is_whitespace() && depth >= 0 {
            any = true;
        }
    }
    None
}

/// `(class, method, arity)` of every Java static the runtime calls.
fn calls(src: &str) -> BTreeSet<(String, String, usize)> {
    let mut out = BTreeSet::new();
    // `bridge!("name", [args...])` calls NativeBridge.
    for (i, _) in src.match_indices("bridge!(") {
        let rest = &src[i + "bridge!(".len()..];
        let Some((name, rest)) = literal(rest) else {
            continue;
        };
        let Some(args) = rest.trim_start().strip_prefix(',') else {
            continue;
        };
        if let Some(n) = list_len(args) {
            out.insert(("NativeBridge".to_owned(), name.to_owned(), n));
        }
    }
    // `host!("dev/yog/<Class>", "name", [args...])` / `jcall!(env, ...)`.
    for (i, _) in src.match_indices("\"dev/yog/") {
        let Some((class, rest)) = literal(&src[i..]) else {
            continue;
        };
        let class = &class["dev/yog/".len()..];
        let Some(rest) = rest.trim_start().strip_prefix(',') else {
            continue;
        };
        let Some((name, rest)) = literal(rest) else {
            continue;
        };
        let Some(args) = rest.trim_start().strip_prefix(',') else {
            continue;
        };
        if let Some(n) = list_len(args) {
            out.insert((class.to_owned(), name.to_owned(), n));
        }
    }
    out
}

/// `(name, arity)` of the public static methods in a Java file.
fn java_statics(java: &str) -> BTreeSet<(String, usize)> {
    let mut out = BTreeSet::new();
    for (i, _) in java.match_indices("public static ") {
        let decl = &java[i..];
        let Some(open) = decl.find('(') else { continue };
        let head = &decl[..open];
        if head.contains(['=', ';', '{']) || head.contains(" class ") {
            continue;
        }
        let Some(name) = head.split_whitespace().last() else {
            continue;
        };
        let Some(close) = decl[open..].find(')') else {
            continue;
        };
        let params = &decl[open + 1..open + close];
        // Generic arguments may hold commas: count the top-level ones.
        let (mut depth, mut commas) = (0i32, 0usize);
        for c in params.chars() {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                ',' if depth == 0 => commas += 1,
                _ => {}
            }
        }
        let arity = if params.trim().is_empty() {
            0
        } else {
            commas + 1
        };
        out.insert((name.to_owned(), arity));
    }
    out
}

/// The public statics of class `class` as a host compiles it: its own
/// platform sources, the host's shared sources, then `java-common`.
fn class_statics(
    root: &Path,
    host: &str,
    platform: &Path,
    class: &str,
) -> Option<BTreeSet<(String, usize)>> {
    let file = format!("{class}.java");
    [
        platform.join("src/main/java/dev/yog"),
        root.join(host).join("src/main/java/dev/yog"),
        root.join("java-common/src/main/java/dev/yog"),
    ]
    .iter()
    .find_map(|d| fs::read_to_string(d.join(&file)).ok())
    .map(|j| java_statics(&j))
}

#[test]
fn every_host_has_every_bridge_method() {
    let root = repo();
    if !root.join("fabric").is_dir() {
        return;
    }
    let calls = calls(&runtime_sources());
    assert!(
        calls.len() > 50,
        "found only {} bridge calls — parser broken?",
        calls.len()
    );
    let mut missing = Vec::new();
    for host in HOSTS {
        let Ok(platforms) = fs::read_dir(root.join(host).join("platforms")) else {
            continue;
        };
        for p in platforms {
            let platform = p.unwrap().path();
            for (class, name, arity) in &calls {
                let ok = class_statics(&root, host, &platform, class)
                    .is_some_and(|s| s.contains(&(name.clone(), *arity)));
                if !ok {
                    missing.push(format!("{}: {class}.{name}/{arity}", platform.display()));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "missing Java bridge methods:\n{}",
        missing.join("\n")
    );
}

#[test]
fn argument_count() {
    assert_eq!(list_len("[S(dim), I(pos.x), f(a, b)]"), Some(3));
    assert_eq!(list_len("[]"), Some(0));
    assert_eq!(list_len("[S(x),\n]"), Some(1));
}

/// The other direction: every `native` method a host's `NativeBridge`
/// declares is exported by the runtime (a missing one is an
/// `UnsatisfiedLinkError` the first time the game calls it).
#[test]
fn every_native_method_is_exported() {
    let root = repo();
    if !root.join("fabric").is_dir() {
        return;
    }
    let src = runtime_sources();
    let mut missing = BTreeSet::new();
    let mut seen = 0;
    for host in HOSTS {
        let Ok(platforms) = fs::read_dir(root.join(host).join("platforms")) else {
            continue;
        };
        for p in platforms {
            let file = p.unwrap().path().join("src/main/java/dev/yog/NativeBridge.java");
            let Ok(java) = fs::read_to_string(&file) else {
                continue;
            };
            for line in java.lines() {
                let Some(rest) = line.split(" static native ").nth(1) else {
                    continue;
                };
                let Some(name) = rest.split('(').next().and_then(|s| s.split_whitespace().last()) else {
                    continue;
                };
                seen += 1;
                if !src.contains(&format!("fn Java_dev_yog_NativeBridge_{name}")) {
                    missing.insert(format!("{}: {name}", file.display()));
                }
            }
        }
    }
    assert!(seen > 100, "found only {seen} native methods — parser broken?");
    assert!(missing.is_empty(), "native methods with no runtime export:\n{}", missing.into_iter().collect::<Vec<_>>().join("\n"));
}
