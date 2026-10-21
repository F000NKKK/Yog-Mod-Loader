//! The mod's `.yui` components in the designer. Markup-only ones are
//! expanded in place ([`components`](super::components)); *compiled* ones —
//! whose bindings call the code-behind — are drawn for real by the mod's
//! code: a designer host process ([`host`](super::host)) per mod loads the
//! mod's library built for this machine (`yog designer-lib`) and draws them
//! on request.
//!
//! This side supervises it — rebuilds the library and restarts the host
//! when the mod's sources change (debounced), restarts nothing while they
//! don't — and fits the components into the designer's picture: each
//! compiled instance is laid out by yog-ui as a spacer of the component's
//! (measured) size, painted with a marker color; the marker's fill in the
//! recorded display list is then replaced by the host's drawing of the
//! component. While the library builds, or when it can't be built or
//! loaded, the element is a labelled placeholder box instead ("designer
//! could not load"), never an IDE crash.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use yog_sync::mpsc::{self, Receiver, RecvTimeoutError};
use yog_sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use serde_json::{Value, json};

use super::components::{self, Def};
use super::host::{ComponentInfo, MARK, RenderReq};
use super::raster::DrawOp;
use crate::design::project;

/// Elements the designer draws itself (yog-ui's and yog-kit's); any other
/// PascalCase element is a custom component (as for `yog build`).
const BUILT_IN: &[&str] = &[
    yog_ids::yui::SCREEN,
    yog_ids::yui::CANVAS,
    yog_ids::yui::COLUMN,
    yog_ids::yui::ROW,
    yog_ids::yui::SCROLL_VIEW,
    yog_ids::yui::LABEL,
    yog_ids::yui::BUTTON,
    yog_ids::yui::TEXT_BOX,
    yog_ids::yui::ITEM_SLOT,
    yog_ids::yui::INV_SLOT,
    yog_ids::yui::SLOT,
    yog_ids::yui::PLAYER_INVENTORY,
    yog_ids::yui::IMAGE,
    yog_ids::yui::SPACER,
    yog_ids::yui::SURFACE,
    yog_ids::yui::RECIPE_SLOT,
    yog_ids::yui::RECIPE_RESULT,
    yog_ids::yui::RECIPE_COUNT,
    yog_ids::yui::GRID,
    "RowDefinition",
    "ColumnDefinition",
    yog_ids::yui::TOGGLE_BUTTON,
    yog_ids::yui::LIST_VIEW,
    yog_ids::yui::ITEMS_CONTROL,
    yog_ids::yui::CONTENT_PRESENTER,
    yog_ids::yui_vm::DATA_TEMPLATE,
    "Dialog",
];

/// What every element takes that sizes/places it — kept on the stand-in
/// spacer; the rest (the component's own properties and its style) goes
/// to the host.
const LAYOUT_ATTRS: &[&str] = &[
    "x",
    "y",
    "w",
    "h",
    "min_w",
    "min_h",
    "dock",
    "flex",
    "margin",
    "halign",
    "valign",
    "Grid.Row",
    "Grid.Column",
    "Grid.RowSpan",
    "Grid.ColumnSpan",
    "id",
    "on_click",
    "enabled",
    "visible",
];

/// Quiet time after a source change before the library is rebuilt.
const DEBOUNCE: Duration = Duration::from_millis(800);
/// How long one host answer may take (a component looping forever).
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

pub fn is_component(kind: &str) -> bool {
    !BUILT_IN.contains(&kind)
        && kind.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The designer library's state, as the toolbox shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// The mod has no compiled components (nothing to build).
    None,
    /// Building the library / starting the host.
    Building,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: Status,
    pub error: Option<String>,
    /// Every component of the mod, from its `.yui` files.
    pub components: Vec<ComponentInfo>,
    /// The ones that need the mod's library (the rest are drawn from markup).
    pub compiled: Vec<String>,
    /// Changes whenever the drawing of an instance may: a component `.yui`
    /// edited, the library rebuilt.
    pub revision: String,
    /// The mod's display name (`yog.toml` `name`, else its id) for the toolbox category.
    pub mod_name: String,
}

struct HostProc {
    child: Child,
    stdin: ChildStdin,
    answers: Receiver<String>,
    stderr: Arc<Mutex<String>>,
    lib_copy: PathBuf,
}

impl HostProc {
    fn spawn(lib: &Path, mod_root: &Path) -> Result<(Self, Vec<ComponentInfo>), String> {
        let mut child = super::this_cli()
            .arg(super::host::COMMAND)
            .current_dir(mod_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("can't start the designer host: {e}"))?;
        let stdin = child.stdin.take().ok_or("no host stdin")?;
        let stdout = child.stdout.take().ok_or("no host stdout")?;
        let mut stderr_pipe = child.stderr.take().ok_or("no host stderr")?;
        let (tx, answers) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Some(answer) = line.strip_prefix(MARK)
                    && tx.send(answer.to_string()).is_err()
                {
                    break;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let tail = stderr.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = stderr_pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut t = tail.lock();
                t.push_str(&String::from_utf8_lossy(&buf[..n]));
                if t.len() > 16_000 {
                    let cut = t.len() - 8_000;
                    let cut = (cut..t.len())
                        .find(|i| t.is_char_boundary(*i))
                        .unwrap_or(t.len());
                    t.drain(..cut);
                }
            }
        });
        let mut host = HostProc {
            child,
            stdin,
            answers,
            stderr,
            lib_copy: lib.to_path_buf(),
        };
        let answer = host.ask(
            &json!({"cmd": "load", "lib": lib, "modRoot": mod_root}),
            Duration::from_secs(30),
        )?;
        let components =
            serde_json::from_value(answer.get("components").cloned().unwrap_or_default())
                .map_err(|e| e.to_string())?;
        Ok((host, components))
    }

    fn ask(&mut self, request: &Value, timeout: Duration) -> Result<Value, String> {
        writeln!(self.stdin, "{request}")
            .and_then(|_| self.stdin.flush())
            .map_err(|_| self.died())?;
        match self.answers.recv_timeout(timeout) {
            Ok(line) => {
                let v: Value =
                    serde_json::from_str(&line).map_err(|e| format!("bad host answer: {e}"))?;
                if v.get("ok").and_then(Value::as_bool) == Some(true) {
                    Ok(v)
                } else {
                    Err(v
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("the designer host failed")
                        .to_string())
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                Err("the designer host stopped responding (a component that never returns?) — it was stopped".into())
            }
            Err(RecvTimeoutError::Disconnected) => Err(self.died()),
        }
    }

    fn died(&mut self) -> String {
        let _ = self.child.wait();
        // Give the stderr reader a moment to drain the last words.
        std::thread::sleep(Duration::from_millis(50));
        let tail = self
            .stderr
            .lock()
            .clone();
        let tail: Vec<&str> = tail
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!(
            "the designer host crashed{}",
            if tail.is_empty() {
                String::new()
            } else {
                format!(":\n{}", tail.join("\n"))
            }
        )
    }
}

impl Drop for HostProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.lib_copy);
    }
}

type Drawn = (f32, f32, Vec<DrawOp>);

struct ModState {
    status: Status,
    error: Option<String>,
    /// What the loaded library registered.
    components: Vec<ComponentInfo>,
    mod_name: String,
    /// The sources the current library (or failure) came from.
    built: Option<u64>,
    /// The latest sources seen, and since when (debounce).
    seen: Option<(u64, Instant)>,
    building: bool,
    host: Option<HostProc>,
    generation: u64,
    cache: HashMap<String, Result<Drawn, String>>,
}

impl ModState {
    fn new() -> Self {
        Self {
            status: Status::None,
            error: None,
            components: Vec::new(),
            mod_name: String::new(),
            built: None,
            seen: None,
            building: false,
            host: None,
            generation: 0,
            cache: HashMap::new(),
        }
    }

    fn snapshot(&self, (rev, defs): &(u64, Arc<Vec<Def>>)) -> Snapshot {
        Snapshot {
            revision: format!("{rev:x}.{}", self.generation),
            status: self.status,
            error: self.error.clone(),
            components: defs.iter().map(|d| d.info.clone()).collect(),
            compiled: defs
                .iter()
                .filter(|d| d.compiled)
                .map(|d| d.info.name.clone())
                .collect(),
            mod_name: self.mod_name.clone(),
        }
    }

    fn render(&mut self, req: &RenderReq) -> Result<Drawn, String> {
        let key = serde_json::to_string(req).unwrap_or_default();
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let Some(host) = self.host.as_mut() else {
            return Err(self
                .error
                .clone()
                .unwrap_or_else(|| "the designer library isn't loaded".into()));
        };
        let mut request = serde_json::to_value(req).unwrap_or_default();
        request["cmd"] = json!("render");
        let result = host.ask(&request, ANSWER_TIMEOUT).and_then(|v| {
            let dim = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let ops: Vec<DrawOp> =
                serde_json::from_value(v.get("ops").cloned().unwrap_or_default())
                    .map_err(|e| e.to_string())?;
            Ok((dim("w"), dim("h"), ops))
        });
        if result.is_err() && host.child.try_wait().ok().flatten().is_some() {
            // The host is gone: say so until the next build brings a new one.
            self.host = None;
            self.status = Status::Failed;
            self.error = result.clone().err();
            self.cache.clear();
            return result;
        }
        if self.cache.len() > 512 {
            self.cache.clear();
        }
        self.cache.insert(key, result.clone());
        result
    }
}

type Shared = Arc<Mutex<ModState>>;

fn mods() -> &'static Mutex<HashMap<PathBuf, Shared>> {
    static M: OnceLock<Mutex<HashMap<PathBuf, Shared>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

fn state(root: &Path) -> Shared {
    mods()
        .lock()
        .entry(root.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(ModState::new())))
        .clone()
}

/// The mod's Rust sources, `yog.toml` and resources — what the library is built from.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![project::path(root)];
    for dir in [yog_ids::project::SRC, yog_ids::project::RESOURCES] {
        let walk = ignore::WalkBuilder::new(root.join(dir))
            .hidden(false)
            .build();
        for entry in walk.flatten() {
            let p = entry.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or_default();
            if p.is_file() && matches!(ext, "rs" | "yui" | "toml" | "yres" | "json" | "png") {
                out.push(p.to_path_buf());
            }
        }
    }
    out.sort();
    out
}

fn fingerprint(files: &[PathBuf]) -> u64 {
    let mut h = DefaultHasher::new();
    for f in files {
        f.hash(&mut h);
        if let Ok(m) = std::fs::metadata(f) {
            m.len().hash(&mut h);
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .hash(&mut h);
        }
    }
    h.finish()
}

fn mod_name(root: &Path) -> String {
    let manifest = crate::manifest::YogToml::read(&project::path(root)).ok();
    // `@Strings.x` names a resource text — the id reads better than the reference.
    manifest
        .as_ref()
        .map(|m| m.name.clone())
        .filter(|n| !n.is_empty() && !n.starts_with(yog_ids::sep::TYPED))
        .or_else(|| manifest.map(|m| m.id).filter(|id| !id.is_empty()))
        .unwrap_or_else(|| {
            root.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// The mod's components and the library's state; starts a rebuild (and
/// host restart) when the sources changed and have been quiet a moment —
/// only if some component is compiled. Cheap enough to poll.
pub fn status(root: &Path) -> Snapshot {
    let defs = components::scan_with_revision(root);
    let shared = state(root);
    let files = sources(root);
    let fp = fingerprint(&files);
    let mut s = shared.lock();
    if s.mod_name.is_empty() {
        s.mod_name = mod_name(root);
    }
    if s.building || s.built == Some(fp) {
        return s.snapshot(&defs);
    }
    if !defs.1.iter().any(|d| d.compiled) {
        s.status = Status::None;
        s.error = None;
        s.components.clear();
        s.host = None;
        s.built = Some(fp);
        return s.snapshot(&defs);
    }
    let now = Instant::now();
    match s.seen {
        Some((seen, since)) if seen == fp => {
            if now.duration_since(since) < DEBOUNCE && s.built.is_some() {
                return s.snapshot(&defs);
            }
        }
        _ => {
            s.seen = Some((fp, now));
            if s.built.is_some() {
                return s.snapshot(&defs);
            }
        }
    }
    s.building = true;
    s.status = Status::Building;
    s.mod_name = mod_name(root);
    let snapshot = s.snapshot(&defs);
    drop(s);
    let root = root.to_path_buf();
    std::thread::spawn(move || rebuild(&root, &shared, fp));
    snapshot
}

/// `yog designer-lib`, then a fresh host on a copy of the library (cargo
/// may overwrite the original while the host has it loaded).
fn rebuild(root: &Path, shared: &Shared, fp: u64) {
    let result = build_lib(root).and_then(|lib| {
        let generation = shared.lock().generation + 1;
        let copy = std::env::temp_dir().join("yog-idle-designer").join(format!(
            "{}-{}-{generation}{}",
            lib.file_stem().unwrap_or_default().to_string_lossy(),
            std::process::id(),
            lib.extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(copy.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        std::fs::copy(&lib, &copy).map_err(|e| format!("copying {}: {e}", lib.display()))?;
        HostProc::spawn(&copy, root).inspect_err(|_| {
            let _ = std::fs::remove_file(&copy);
        })
    });
    let mut s = shared.lock();
    s.building = false;
    s.built = Some(fp);
    s.generation += 1;
    s.cache.clear();
    match result {
        Ok((host, components)) => {
            s.host = Some(host);
            s.components = components;
            s.status = Status::Ready;
            s.error = None;
        }
        Err(e) => {
            s.host = None;
            s.status = Status::Failed;
            s.error = Some(e);
        }
    }
}

fn build_lib(root: &Path) -> Result<PathBuf, String> {
    let out = crate::design::this_cli()
        .arg("designer-lib")
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run yog: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        let path = stdout
            .lines()
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .unwrap_or_default();
        return if Path::new(path).is_file() {
            Ok(PathBuf::from(path))
        } else {
            Err(format!("yog designer-lib gave no library path:\n{stdout}"))
        };
    }
    if stderr.contains("unknown command: designer-lib") {
        return Err("this yog CLI can't build designer libraries — update it (Tools → Environment, or `cargo install --locked yog-cli`)".into());
    }
    Err(build_errors(&stderr))
}

/// Cargo's errors (each with its location), else the output's tail.
fn build_errors(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("error") && !l.starts_with("error: could not compile") {
            out.push(l.to_string());
            if let Some(at) = lines
                .get(i + 1)
                .filter(|n| n.trim_start().starts_with("-->"))
            {
                out.push(at.trim().to_string());
            }
        }
    }
    if out.is_empty() {
        out = lines
            .iter()
            .rev()
            .take(15)
            .rev()
            .map(|l| l.to_string())
            .collect();
    }
    let text = out.join("\n");
    if text.trim().is_empty() {
        "building the designer library failed".into()
    } else {
        text.chars().take(4000).collect()
    }
}

mod fit;
mod placeholder;
#[cfg(test)]
mod tests;

pub use fit::{Prepared, prepare, splice};
pub use placeholder::placeholder;
