//! Finding where a `dlopen`'d native actually landed in a live process's
//! address space — `yog-symbols`' addresses are relative to the unstripped
//! native's own file, but breakpoints need the real runtime address, which
//! is `module_base + offset`.

use std::path::{Path, PathBuf};

use nix::unistd::Pid;

/// Every file-backed mapping in `/proc/<pid>/maps`, in address order, as
/// `(start address, backing path)`. Empty if the file can't be read.
pub(crate) fn file_mappings(pid: i32) -> Vec<(u64, PathBuf)> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).unwrap_or_default();
    maps.lines()
        .filter_map(|line| {
            // start-end perms offset dev inode [pathname]
            let fields: Vec<&str> = line.split_whitespace().collect();
            let path = fields.get(5..)?.join(" ");
            if path.is_empty() {
                return None;
            }
            let start = u64::from_str_radix(fields[0].split('-').next()?, 16).ok()?;
            Some((start, PathBuf::from(path)))
        })
        .collect()
}

/// The start of the first mapping in `pid` whose backing file's name
/// satisfies `matches`.
fn first_mapping(pid: Pid, matches: impl Fn(&Path) -> bool) -> Option<u64> {
    file_mappings(pid.as_raw())
        .into_iter()
        .find(|(_, path)| matches(path))
        .map(|(start, _)| start)
}

/// The load base of the mapping in `pid` whose backing file matches
/// `module_path` (compared by canonical path, falling back to a filename
/// match since a process may see a different mount namespace view).
pub fn find_module_base(pid: Pid, module_path: &Path) -> Option<u64> {
    let canonical = module_path.canonicalize().ok();
    let file_name = module_path.file_name();
    first_mapping(pid, |mapped| {
        canonical.as_deref() == Some(mapped)
            || file_name.is_some_and(|f| mapped.file_name() == Some(f))
    })
}

/// Like [`find_module_base`], but matches a mapping whose filename merely
/// *starts with* `prefix` — for natives whose exact on-disk name isn't
/// predictable from outside (`yog-runtime`'s `extract_yog` embeds its own
/// pid in the extracted filename: `yog-<mod-yog-stem>-<pid>.<ext>`). Pass
/// e.g. `"yog-my-mod-"` to find that mod's extracted native regardless of
/// the embedded pid.
pub fn find_module_base_by_prefix(pid: Pid, prefix: &str) -> Option<u64> {
    first_mapping(pid, |mapped| {
        mapped
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|f| f.starts_with(prefix))
    })
}
