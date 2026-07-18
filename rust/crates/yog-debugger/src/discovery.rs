//! Finding the real process to attach to when the one a caller actually
//! spawned is a launcher wrapper (`./gradlew runClient`, a Gradle daemon,
//! ...) that forks its own JVM — often several layers deep — rather than
//! becoming the game process itself. Attaching to the wrapper's own pid
//! would attach to the wrong process entirely; this walks its descendants
//! looking for the one that actually has the runtime loaded.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use nix::unistd::Pid;

/// Parses `/proc/<pid>/stat`'s 4th whitespace-separated field (ppid).
/// The 2nd field is `(comm)` and may itself contain spaces/parens, so this
/// splits on the *last* `)` first rather than naively splitting the whole
/// line on whitespace.
fn parent_pid(pid: i32) -> Option<i32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(')')?.1;
    after_comm.split_whitespace().nth(1)?.parse().ok()
}

/// Every currently-visible pid, by listing `/proc`.
fn all_pids() -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<i32>().ok()))
        .collect()
}

/// Whether any mapping in `pid`'s `/proc/<pid>/maps` has a filename
/// containing `needle` (e.g. `"yog_runtime"` to match `libyog_runtime.so`
/// regardless of platform-specific prefix/extension).
fn has_mapping_containing(pid: i32, needle: &str) -> bool {
    let Ok(maps) = std::fs::read_to_string(format!("/proc/{pid}/maps")) else { return false };
    maps.lines().any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 6 {
            return false;
        }
        let path = fields[5..].join(" ");
        Path::new(&path).file_name().and_then(|f| f.to_str()).is_some_and(|f| f.contains(needle))
    })
}

/// Breadth-first search over every descendant of `root_pid` (however many
/// launcher/wrapper/daemon layers deep) for the first one with a mapping
/// whose filename contains `needle`. `root_pid` itself is checked too, in
/// case the caller's own spawned process turned out to be the real one.
pub fn find_descendant_with_module(root_pid: Pid, needle: &str) -> Option<Pid> {
    let root = root_pid.as_raw();

    // Build the full pid->ppid map once, then walk it — cheaper than
    // re-scanning /proc per BFS level, and avoids missing children whose
    // parent already exited (reparented to init) between scans.
    let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
    for pid in all_pids() {
        if let Some(ppid) = parent_pid(pid) {
            children.entry(ppid).or_default().push(pid);
        }
    }

    let mut queue: VecDeque<i32> = VecDeque::from([root]);
    let mut seen: HashSet<i32> = HashSet::new();
    while let Some(pid) = queue.pop_front() {
        if !seen.insert(pid) {
            continue;
        }
        if has_mapping_containing(pid, needle) {
            return Some(Pid::from_raw(pid));
        }
        if let Some(kids) = children.get(&pid) {
            queue.extend(kids);
        }
    }
    None
}

/// Scans every process visible to this user for one with a mapping whose
/// filename contains `needle` — no ancestry check at all, unlike
/// [`find_descendant_with_module`]. Needed because a launcher wrapper isn't
/// always a genuine ancestor of the process it "launches": Gradle's daemon
/// model in particular means `./gradlew runClient` commonly just talks to
/// an *already-running*, long-lived daemon over a socket — the JVM that
/// actually loads `yog-runtime` may be parented to that daemon (started by
/// some earlier, unrelated `gradlew` invocation), never to the `./gradlew`
/// process this session spawned, so no amount of walking that process's
/// descendants would ever find it. This is a deliberately broader,
/// last-resort fallback for exactly that case — call it only after
/// [`find_descendant_with_module`] has failed, since matching by loaded
/// module name alone (not ancestry) could in principle pick up an
/// unrelated concurrently-running Yog instance if more than one exists.
pub fn find_process_with_module(needle: &str) -> Option<Pid> {
    all_pids().into_iter().find(|&pid| has_mapping_containing(pid, needle)).map(Pid::from_raw)
}
