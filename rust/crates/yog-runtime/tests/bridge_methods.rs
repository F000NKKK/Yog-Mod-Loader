//! Every Java method the runtime calls — and every host method an action
//! names (`yog_event::services`) — exists, with the arguments it's called
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

/// Every source of the runtime (its folders' too), one after another.
fn runtime_sources() -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    yog_analysis::files::with_ext(&src, &["rs"])
        .iter()
        .map(|p| fs::read_to_string(p).unwrap())
        .collect()
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

/// How many arguments a value of `shape` spreads into when the host calls
/// an action's method: a struct its fields', each spread; anything else one.
fn spread(shape: &yog_data::Shape) -> usize {
    match shape {
        yog_data::Shape::Struct(s) => s.fields.iter().map(|f| spread(&(f.shape)())).sum(),
        _ => 1,
    }
}

/// `(class, method, arity)` of every host method the actions name: a
/// `NativeBridge` method, or `Class.method` of another `dev.yog` class.
fn action_calls() -> BTreeSet<(String, String, usize)> {
    yog_event::services()
        .iter()
        .filter_map(|s| {
            let (class, method) = s
                .host?
                .rsplit_once('.')
                .unwrap_or(("NativeBridge", s.host?));
            Some((class.to_owned(), method.to_owned(), spread(&s.shape)))
        })
        .collect()
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

/// A platform's `<host>/versions/<mc>.properties` value of `key`.
fn platform_property(root: &Path, host: &str, platform: &Path, key: &str) -> Option<String> {
    let mc = platform.file_name()?.to_str()?;
    let text = fs::read_to_string(
        root.join(host)
            .join("versions")
            .join(format!("{mc}.properties")),
    )
    .ok()?;
    text.lines().find_map(|l| {
        l.trim()
            .strip_prefix(key)?
            .trim_start()
            .strip_prefix('=')
            .map(|v| v.trim().to_string())
    })
}

/// Whether a platform is only build plumbing so far (`status=scaffold`): the build refuses it.
fn scaffold(root: &Path, host: &str, platform: &Path) -> bool {
    platform_property(root, host, platform, "status").as_deref() == Some("scaffold")
}

/// A version's numeric parts (`1.21.1` → `[1, 21, 1]`); `None` if it isn't one.
fn version_parts(v: &str) -> Option<Vec<u32>> {
    v.split('.').map(|p| p.parse().ok()).collect()
}

/// Whether `v` is at least `lo` and within `hi` on as many parts as `hi` names — `sources.gradle`'s rule.
fn in_range(v: &[u32], lo: &[u32], hi: &[u32]) -> bool {
    let compare = |a: &[u32], b: &[u32], n: usize| {
        (0..n)
            .map(|i| a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    compare(v, lo, v.len().max(lo.len())).is_ge() && compare(v, hi, hi.len()).is_le()
}

/// The Java source folders a host compiles for a platform: the platform's
/// own, the host's shared ones (not for a platform built by a Gradle
/// project of its own, `gradle_project`), `java-common`'s, and each
/// `java-common/mc/<range>` holding its version — as `java-common/sources.gradle` picks them.
fn source_dirs(root: &Path, host: &str, platform: &Path) -> Vec<PathBuf> {
    const JAVA: &str = "src/main/java";
    let shared = platform_property(root, host, platform, "gradle_project").is_none();
    let mut dirs = vec![platform.join(JAVA)];
    if shared {
        dirs.push(root.join(host).join(JAVA));
    }
    dirs.push(root.join("java-common").join(JAVA));
    let Some(v) = platform
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(version_parts)
    else {
        return dirs;
    };
    let mut ranges: Vec<PathBuf> = fs::read_dir(root.join("java-common/mc"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    ranges.sort();
    for d in ranges {
        let Some(name) = d.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let (lo, hi) = name.split_once('-').unwrap_or((name, name));
        if let (Some(lo), Some(hi)) = (version_parts(lo), version_parts(hi))
            && in_range(&v, &lo, &hi)
        {
            dirs.push(d.join(JAVA));
        }
    }
    dirs
}

/// The public statics of class `class` as a host compiles it for `platform`.
fn class_statics(
    root: &Path,
    host: &str,
    platform: &Path,
    class: &str,
) -> Option<BTreeSet<(String, usize)>> {
    let file = format!("dev/yog/{}.java", class.replace('.', "/"));
    let found: Vec<_> = source_dirs(root, host, platform)
        .into_iter()
        .filter_map(|d| fs::read_to_string(d.join(&file)).ok())
        .collect();
    (!found.is_empty()).then(|| found.iter().flat_map(|j| java_statics(j)).collect())
}

#[test]
fn every_host_has_every_bridge_method() {
    let root = repo();
    if !root.join("fabric").is_dir() {
        return;
    }
    let mut calls = calls(&runtime_sources());
    calls.extend(action_calls());
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
            if scaffold(&root, host, &platform) {
                continue;
            }
            for (class, name, arity) in &calls {
                let ok = class_statics(&root, host, &platform, class)
                    .is_some_and(|s| s.contains(&(name.clone(), *arity)));
                if !ok {
                    missing.push(format!("{}: {class}.{name}/{arity}", platform.display()));
                }
            }
            // The host finds an action's method by its name and arity: one of them.
            for (class, name, arity) in action_calls() {
                let Some(statics) = class_statics(&root, host, &platform, &class) else {
                    continue;
                };
                let found = statics
                    .iter()
                    .filter(|(n, a)| *n == name && *a == arity)
                    .count();
                if found > 1 {
                    missing.push(format!(
                        "{}: {class}.{name}/{arity} is overloaded",
                        platform.display()
                    ));
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
fn an_action_spreads_its_positions() {
    let set_block = action_calls()
        .into_iter()
        .find(|(_, name, _)| name == "setBlock");
    assert_eq!(set_block.map(|(_, _, n)| n), Some(5), "dim, x, y, z, block");
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
            let platform = p.unwrap().path();
            let Some((file, java)) = source_dirs(&root, host, &platform)
                .into_iter()
                .map(|d| d.join("dev/yog/NativeBridge.java"))
                .find_map(|f| fs::read_to_string(&f).ok().map(|j| (f, j)))
            else {
                continue;
            };
            for line in java.lines() {
                let Some(rest) = line.split(" static native ").nth(1) else {
                    continue;
                };
                let Some(name) = rest
                    .split('(')
                    .next()
                    .and_then(|s| s.split_whitespace().last())
                else {
                    continue;
                };
                seen += 1;
                if !src.contains(&format!("fn Java_dev_yog_NativeBridge_{name}")) {
                    missing.insert(format!("{}: {name}", file.display()));
                }
            }
        }
    }
    assert!(
        seen > 100,
        "found only {seen} native methods — parser broken?"
    );
    assert!(
        missing.is_empty(),
        "native methods with no runtime export:\n{}",
        missing.into_iter().collect::<Vec<_>>().join("\n")
    );
}

/// `YogBus.java`'s status values are `yog_abi::status`'s.
#[test]
fn java_bus_statuses_match_the_abi() {
    let java = repo().join("java-common/src/main/java/dev/yog/YogBus.java");
    let Ok(src) = fs::read_to_string(&java) else {
        return;
    };
    let value = |name: &str| -> Option<u32> {
        let line = src
            .lines()
            .find(|l| l.contains(&format!("public static final int {name} =")))?;
        line.split('=')
            .nth(1)?
            .trim()
            .trim_end_matches(';')
            .parse()
            .ok()
    };
    use yog_abi::status::*;
    for (name, v) in [
        ("OK", OK),
        ("NO_HANDLER", NO_HANDLER),
        ("BAD_PAYLOAD", BAD_PAYLOAD),
        ("FAILED", FAILED),
        ("CANCELLED", CANCELLED),
    ] {
        assert_eq!(value(name), Some(v), "YogBus.{name}");
    }
}
