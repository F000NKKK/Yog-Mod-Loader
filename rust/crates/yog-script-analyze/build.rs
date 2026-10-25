//! The System library's sources: every `.ys` under `src/system/`, one type
//! a file, its folders its namespace (`System/Collections/Generic/List.ys`)
//! — listed here so no list is kept by hand (`src/system.rs` includes it).

use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// Where the library's sources are, from the crate's root.
const LIBRARY: &str = "src/system";
/// A Yog Script source's extension.
const SOURCE_EXT: &str = "ys";
/// What the list is written to, in the build's output folder.
const LIST: &str = "system_sources.rs";

fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default()).join(LIBRARY);
    println!("cargo:rerun-if-changed={LIBRARY}");
    let mut sources = Vec::new();
    collect(&root, &mut sources)?;
    sources.sort();
    let mut out = String::from(
        "/// The System library's sources: (path under `src/system/`, text).\n\
         pub(crate) const SOURCES: &[(&str, &str)] = &[\n",
    );
    for path in &sources {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push_str(&format!(
            "    ({rel:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    out.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default()).join(LIST),
        out,
    )
}

/// Every source under `dir`, each folder watched for new ones.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == SOURCE_EXT) {
            out.push(path);
        }
    }
    Ok(())
}
