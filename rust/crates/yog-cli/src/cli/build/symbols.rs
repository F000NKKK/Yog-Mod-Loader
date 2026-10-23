//! What a mod's native keeps of its debug info: none (a plain build), all
//! of it unoptimized (`yog build debug`, for stepping through), or a
//! release's — optimized, the debug info split out beside the `.yog`
//! (`yog build release`: `artifacts/symbols/<platform>/<lib>.debug`, what a
//! crash's stack is read with), the packaged native without it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What the native keeps of its debug info.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Symbols {
    Stripped,
    Kept,
    Split,
}

/// `yog build debug`, `yog build release`.
const KEPT_WORD: &str = "debug";
const SPLIT_WORD: &str = "release";
/// Where a release's split debug info goes, under the artifacts, and its extension.
pub(crate) const SYMBOLS_DIR: &str = yog_ids::symbols::DIR;
const DEBUG_EXT: &str = yog_ids::symbols::EXT;
/// Where the packaged (stripped) copies go, under `.yog-build/`.
const STRIPPED_DIR: &str = "stripped";
/// The linker-agnostic objcopy every Rust toolchain ships (LLVM's).
const OBJCOPY: &str = "rust-objcopy";

impl Symbols {
    /// The word after `yog build` naming it.
    pub(crate) fn of(word: Option<&str>) -> Symbols {
        match word {
            Some(KEPT_WORD) => Symbols::Kept,
            Some(SPLIT_WORD) => Symbols::Split,
            _ => Symbols::Stripped,
        }
    }

    /// The word passed on to another `yog build` (none for a plain one).
    pub(crate) fn word(self) -> Option<&'static str> {
        match self {
            Symbols::Stripped => None,
            Symbols::Kept => Some(KEPT_WORD),
            Symbols::Split => Some(SPLIT_WORD),
        }
    }

    /// Cargo's release profile changed for it (a plain build keeps the default).
    pub(crate) fn profile(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Symbols::Stripped => &[],
            // A debug build in all but name: inlining and scheduling make a
            // debugger's line steps hop between files and back.
            Symbols::Kept => &[
                ("CARGO_PROFILE_RELEASE_DEBUG", "true"),
                ("CARGO_PROFILE_RELEASE_STRIP", "none"),
                ("CARGO_PROFILE_RELEASE_OPT_LEVEL", "0"),
                ("CARGO_PROFILE_RELEASE_LTO", "false"),
                ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "256"),
                ("CARGO_PROFILE_RELEASE_INCREMENTAL", "true"),
            ],
            // Optimized as ever, the debug info kept to be split out.
            Symbols::Split => &[
                ("CARGO_PROFILE_RELEASE_DEBUG", "true"),
                ("CARGO_PROFILE_RELEASE_STRIP", "none"),
            ],
        }
    }
}

/// A release's natives as packaged: each one's debug info written to
/// `artifacts/symbols/<tag>/<lib>.debug` and a stripped copy (under
/// `build_dir`) in its place. The built file stays as it is, so building
/// again with nothing changed splits it again whole.
pub(crate) fn split(
    bundled: Vec<(String, PathBuf)>,
    build_dir: &Path,
    artifacts: &Path,
) -> Result<Vec<(String, PathBuf)>, String> {
    let objcopy = objcopy()?;
    bundled
        .into_iter()
        .map(|(tag, native)| {
            let name = native.file_name().ok_or("a native without a name")?;
            let symbols = artifacts.join(SYMBOLS_DIR).join(&tag);
            let stripped = build_dir.join(STRIPPED_DIR).join(&tag);
            for dir in [&symbols, &stripped] {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let debug = symbols.join(name).with_added_extension(DEBUG_EXT);
            let out = stripped.join(name);
            run(
                &objcopy,
                &[
                    "--only-keep-debug".as_ref(),
                    native.as_os_str(),
                    debug.as_os_str(),
                ],
            )?;
            run(
                &objcopy,
                &[
                    "--strip-debug".as_ref(),
                    native.as_os_str(),
                    out.as_os_str(),
                ],
            )?;
            Ok((tag, out))
        })
        .collect()
}

fn run(program: &Path, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let ok = Command::new(program)
        .args(args)
        .status()
        .is_ok_and(|s| s.success());
    ok.then_some(())
        .ok_or_else(|| format!("{} {:?} failed", program.display(), args))
}

/// The toolchain's `rust-objcopy` (`<sysroot>/lib/rustlib/<host>/bin/`).
fn objcopy() -> Result<PathBuf, String> {
    let out = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .map_err(|e| e.to_string())?;
    let sysroot = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let rustlib = sysroot.join("lib").join("rustlib");
    let file = format!("{OBJCOPY}{}", std::env::consts::EXE_SUFFIX);
    std::fs::read_dir(&rustlib)
        .map_err(|e| format!("{}: {e}", rustlib.display()))?
        .flatten()
        .map(|host| host.path().join("bin").join(&file))
        .find(|p| p.is_file())
        .ok_or_else(|| {
            format!(
                "no {OBJCOPY} in {} — the Rust toolchain ships it",
                rustlib.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This test's own program (built with debug info): split, its debug
    /// info is a file of its own and the packaged copy is smaller.
    #[test]
    fn a_release_native_is_split_from_its_debug_info() {
        let exe = std::env::current_exe().unwrap();
        let dir = std::env::temp_dir().join(format!("yog-symbols-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tag = "test-platform".to_string();
        let out = split(
            vec![(tag.clone(), exe.clone())],
            &dir.join("build"),
            &dir.join("artifacts"),
        )
        .unwrap();
        let size = |p: &Path| std::fs::metadata(p).unwrap().len();
        let name = exe.file_name().unwrap();
        let debug = dir
            .join("artifacts")
            .join(SYMBOLS_DIR)
            .join(&tag)
            .join(name)
            .with_added_extension(DEBUG_EXT);
        assert!(size(&debug) > 0);
        assert!(
            size(&out[0].1) < size(&exe),
            "the packaged copy lost its debug info"
        );
        assert!(exe.is_file(), "the built file stays");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
