//! Finding where a `dlopen`'d native actually landed in a live process's
//! address space — `yog-symbols`' addresses are relative to the unstripped
//! native's own file, but breakpoints need the real runtime address, which
//! is `module_base + offset`.

use std::path::Path;

use nix::unistd::Pid;

/// The load base of the mapping in `pid` whose backing file matches
/// `module_path` (compared by canonical path, falling back to a filename
/// match since a process may see a different mount namespace view).
pub fn find_module_base(pid: Pid, module_path: &Path) -> Option<u64> {
    let maps = std::fs::read_to_string(format!("/proc/{}/maps", pid.as_raw())).ok()?;
    let canonical = module_path.canonicalize().ok();
    let file_name = module_path.file_name();

    for line in maps.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // start-end perms offset dev inode [pathname]
        if fields.len() < 6 {
            continue;
        }
        let path = fields[5..].join(" ");
        if path.is_empty() {
            continue;
        }

        let mapped = Path::new(&path);
        let matches = canonical.as_deref().is_some_and(|c| c == mapped) || file_name.is_some_and(|f| mapped.file_name() == Some(f));
        if !matches {
            continue;
        }

        let start_hex = fields[0].split('-').next()?;
        return u64::from_str_radix(start_hex, 16).ok();
    }
    None
}
