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
        match c {
            _ => {}
        }
    }
    None
}

/// `(class, method, arity)` of every Java static the runtime calls.
fn calls(src: &str) -> BTreeSet<(String, String, usize)> {
    let mut out = BTreeSet::new();
    // `bridge_bool("name", &[args...])` and friends call NativeBridge.
    for (i, _) in src.match_indices("bridge_") {
        let rest = &src[i + "bridge_".len()..];
        let Some(open) = rest.find('(') else { continue };
        if open == 0 || !rest[..open].chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let Some((name, rest)) = literal(&rest[open + 1..]) else {
            continue;
        };
        let Some(args) = rest
            .trim_start()
            .strip_prefix(',')
            .and_then(|r| r.trim_start().strip_prefix('&'))
        else {
            continue;
        };
        if let Some(n) = list_len(args) {
            out.insert(("NativeBridge".to_owned(), name.to_owned(), n));
        }
    }
    // `call_static_method("dev/yog/<Class>", "name", "(sig)R", ...)`.
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
        let Some((sig, _)) = rest.trim_start().strip_prefix(',').and_then(literal) else {
            continue;
        };
        out.insert((class.to_owned(), name.to_owned(), sig_arity(sig)));
    }
    out
}

/// Number of parameters in a JNI method signature.
fn sig_arity(sig: &str) -> usize {
    let Some(params) = sig.strip_prefix('(').and_then(|s| s.split(')').next()) else {
        return 0;
    };
    let mut n = 0;
    let mut chars = params.chars();
    while let Some(c) = chars.next() {
        match c {
            '[' => continue,
            'L' => {
                for c in chars.by_ref() {
                    if c == ';' {
                        break;
                    }
                }
                n += 1;
            }
            _ => n += 1,
        }
    }
    n
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
fn signature_arity() {
    assert_eq!(sig_arity("(IIILjava/lang/String;)Z"), 4);
    assert_eq!(sig_arity("()Ljava/lang/String;"), 0);
    assert_eq!(sig_arity("([BLjava/lang/String;D)V"), 3);
}
