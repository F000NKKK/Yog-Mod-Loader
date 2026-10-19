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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use serde_json::{Value, json};

use super::components::{self, Def};
use super::host::{ComponentInfo, MARK, RenderReq};
use super::markup::YuiNode;
use super::raster::DrawOp;
use crate::design::project;
use yog_sync::Locked;

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
    "DataTemplate",
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
                let mut t = tail.locked();
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
            .locked()
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
        .locked()
        .entry(root.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(ModState::new())))
        .clone()
}

fn lock(s: &Shared) -> std::sync::MutexGuard<'_, ModState> {
    s.locked()
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
    let mut s = lock(&shared);
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
        let generation = lock(shared).generation + 1;
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
    let mut s = lock(shared);
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

// ── Fitting components into the picture ────────────────────────────────────────

/// A custom element of the document being drawn.
struct Slot {
    uid: String,
    marker: u32,
    req: RenderReq,
    /// Why it can't be drawn (placeholder text).
    problem: Option<String>,
}

/// The document with custom elements replaced by marked spacers.
pub struct Prepared {
    pub root: YuiNode,
    slots: Vec<Slot>,
    mod_root: Option<PathBuf>,
}

impl Prepared {
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// A component's stand-in color: alpha 1 (never a real color in a .yui,
/// invisible if left behind), a tag, and the component's index.
const MARKER: u32 = 0x01D0_0000;
const MARKER_INDEX: u32 = 0xFFFF;

fn marker(i: usize) -> u32 {
    MARKER | (i as u32 & MARKER_INDEX)
}

fn px(v: Option<&str>) -> Option<f32> {
    let v = v?.trim();
    if v.ends_with('%') {
        None
    } else {
        v.parse().ok()
    }
}

/// The document as yog-ui should draw it: a `<Component>` document as a
/// screen of its design size; markup-only component instances expanded;
/// each compiled one (or unknown) measured through the host (its natural
/// size, when the element doesn't size it in px) and swapped for a marked
/// spacer.
pub fn prepare(root: &YuiNode, mod_root: Option<&Path>, screen_w: f32, screen_h: f32) -> Prepared {
    let root = if root.kind == yog_ids::yui::COMPONENT {
        components::design_root(root)
    } else {
        root.clone()
    };
    let mut prepared = Prepared {
        root,
        slots: Vec::new(),
        mod_root: mod_root.map(Path::to_path_buf),
    };
    if !has_custom(&prepared.root) {
        return prepared;
    }
    let mut cx = Cx {
        mod_root,
        defs: mod_root.map(components::scan).unwrap_or_default(),
        snapshot: None,
        slots: Vec::new(),
        screen: (screen_w, screen_h),
    };
    cx.process(&mut prepared.root, 0);
    prepared.slots = cx.slots;
    prepared
}

/// How deep components may nest (a component using itself stops here).
const MAX_NESTING: usize = 16;

struct Cx<'a> {
    mod_root: Option<&'a Path>,
    defs: Arc<Vec<Def>>,
    snapshot: Option<Snapshot>,
    slots: Vec<Slot>,
    screen: (f32, f32),
}

impl Cx<'_> {
    fn process(&mut self, node: &mut YuiNode, depth: usize) {
        if !is_component(&node.kind) {
            for child in &mut node.children {
                self.process(child, depth);
            }
            return;
        }
        let defs = self.defs.clone();
        match Def::find(&defs, &node.kind) {
            Some(def) if !def.compiled && depth < MAX_NESTING => {
                *node = components::expand(def, node);
                // The body's root may itself be a component.
                self.process(node, depth + 1);
            }
            def => {
                let problem = match def {
                    None => Some(format!(
                        "no component <{}> in this mod — add one: New Item → Component (.yui)",
                        node.kind
                    )),
                    Some(d) if !d.compiled => Some("this component contains itself".to_string()),
                    Some(_) => self.library_problem(&node.kind),
                };
                self.slot(node, problem);
            }
        }
    }

    /// Why a compiled component can't be drawn right now, if it can't.
    fn library_problem(&mut self, name: &str) -> Option<String> {
        let Some(mod_root) = self.mod_root else {
            return Some("not inside a mod (no src/ folder)".into());
        };
        let s = self.snapshot.get_or_insert_with(|| status(mod_root));
        match s.status {
            Status::Building | Status::None => Some(format!("{BUILDING} designer library…")),
            Status::Failed => Some(format!(
                "The designer could not load the mod's library:\n{}",
                s.error.clone().unwrap_or_default()
            )),
            Status::Ready => {
                let registered = lock(&state(mod_root))
                    .components
                    .iter()
                    .any(|c| c.name == name);
                (!registered).then(|| format!("the mod's library doesn't register <{name}> — does its code-behind call yog_api::ui_include!?"))
            }
        }
    }

    fn slot(&mut self, node: &mut YuiNode, problem: Option<String>) {
        let name = node.kind.clone();
        let props: Vec<(String, String)> = node
            .attrs
            .iter()
            .filter(|(k, _)| !LAYOUT_ATTRS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let (w, h) = (px(node.attr(yog_ids::yui_attr::W)), px(node.attr(yog_ids::yui_attr::H)));
        let req = RenderReq {
            name,
            props,
            w: w.unwrap_or(0.0),
            h: h.unwrap_or(0.0),
            avail_w: self.screen.0,
            avail_h: self.screen.1,
        };
        let (mut nw, mut nh) = (
            w.unwrap_or(PLACEHOLDER_W),
            h.unwrap_or(if problem.is_some() { PROBLEM_H } else { PLACEHOLDER_H }),
        );
        let problem = match (problem, self.mod_root) {
            (None, Some(root)) if w.is_none() || h.is_none() => {
                match lock(&state(root)).render(&req) {
                    Ok((mw, mh, _)) => {
                        nw = w.unwrap_or(mw);
                        nh = h.unwrap_or(mh);
                        None
                    }
                    Err(e) => Some(e),
                }
            }
            (p, _) => p,
        };
        let i = self.slots.len();
        let mut attrs: HashMap<String, String> = node
            .attrs
            .iter()
            .filter(|(k, _)| LAYOUT_ATTRS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        attrs.entry(yog_ids::yui_attr::W.into()).or_insert_with(|| nw.to_string());
        attrs.entry(yog_ids::yui_attr::H.into()).or_insert_with(|| nh.to_string());
        attrs.insert(yog_ids::yui_attr::BG.into(), format!("#{:08X}", marker(i)));
        self.slots.push(Slot {
            uid: node.uid.clone(),
            marker: marker(i),
            req,
            problem,
        });
        *node = YuiNode {
            uid: node.uid.clone(),
            kind: yog_ids::yui::SPACER.into(),
            attrs,
            children: Vec::new(),
        };
    }
}

fn has_custom(node: &YuiNode) -> bool {
    is_component(&node.kind) || node.children.iter().any(has_custom)
}

/// Replaces each marker fill in `ops` with the component drawn at its
/// element's laid-out rect (`rects`: uid → `x y w h`), clipped to the part
/// that was visible (the marker fill, already cut to scroll views).
pub fn splice(ops: &mut Vec<DrawOp>, prepared: &Prepared, rects: &HashMap<String, [f32; 4]>) {
    if prepared.slots.is_empty() {
        return;
    }
    let shared = prepared.mod_root.as_deref().map(state);
    let mut out = Vec::with_capacity(ops.len());
    for op in ops.drain(..) {
        let DrawOp::Rect {
            x1,
            y1,
            x2,
            y2,
            color,
        } = op
        else {
            out.push(op);
            continue;
        };
        let Some(slot) = prepared.slots.iter().find(|s| s.marker == color) else {
            out.push(op);
            continue;
        };
        let [x, y, w, h] = rects
            .get(&slot.uid)
            .copied()
            .unwrap_or([x1, y1, x2 - x1, y2 - y1]);
        let drawn = match (&slot.problem, &shared) {
            (Some(p), _) => Err(p.clone()),
            (None, None) => Err("not inside a mod".into()),
            (None, Some(sh)) => lock(sh)
                .render(&RenderReq {
                    w,
                    h,
                    ..slot.req.clone()
                })
                .map(|(_, _, ops)| ops),
        };
        let inner = drawn.unwrap_or_else(|e| placeholder(&slot.req.name, &e, w, h));
        out.push(DrawOp::Scissor {
            x: x1.floor() as i32,
            y: y1.floor() as i32,
            w: (x2.ceil() - x1.floor()) as i32,
            h: (y2.ceil() - y1.floor()) as i32,
        });
        out.extend(inner.into_iter().map(|o| translate(o, x, y)));
        out.push(DrawOp::ClearScissor);
    }
    *ops = out;
}

fn translate(op: DrawOp, dx: f32, dy: f32) -> DrawOp {
    match op {
        DrawOp::Svg { data, x, y, w, h } => DrawOp::Svg { data, x: x + dx, y: y + dy, w, h },
        DrawOp::TextScaled { text, font, x, y, scale, color, shadow } => {
            DrawOp::TextScaled { text, font, x: x + dx, y: y + dy, scale, color, shadow }
        }
        DrawOp::Blocks { list, x, y, w, h, yaw, pitch } => DrawOp::Blocks { list, x: x + dx, y: y + dy, w, h, yaw, pitch },
        DrawOp::Entity { entity, x, y, w, h } => DrawOp::Entity { entity, x: x + dx, y: y + dy, w, h },
        DrawOp::Rect {
            x1,
            y1,
            x2,
            y2,
            color,
        } => DrawOp::Rect {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
            color,
        },
        DrawOp::Gradient {
            x1,
            y1,
            x2,
            y2,
            top,
            bottom,
        } => DrawOp::Gradient {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
            top,
            bottom,
        },
        DrawOp::Text {
            text,
            font,
            x,
            y,
            color,
            shadow,
        } => DrawOp::Text {
            text,
            font,
            x: x + dx,
            y: y + dy,
            color,
            shadow,
        },
        DrawOp::Item { id, x, y, size } => DrawOp::Item {
            id,
            x: x + dx,
            y: y + dy,
            size,
        },
        DrawOp::Image {
            source,
            x,
            y,
            w,
            h,
            region,
            tint,
        } => DrawOp::Image {
            source,
            x: x + dx,
            y: y + dy,
            w,
            h,
            region,
            tint,
        },
        // yog-ui clips in software; a component's own scissor would undo the slot's.
        DrawOp::Scissor { .. } | DrawOp::ClearScissor => DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
            color: 0,
        },
    }
}

/// A placeholder's size when its element gives none, and its look: a dark
/// box, its border and message bluish while the library builds, red when
/// it failed; text padded, a line apart, wrapped at a few columns at least.
const PLACEHOLDER_W: f32 = 80.0;
const PLACEHOLDER_H: f32 = 20.0;
/// A placeholder with a problem to tell: a little taller.
const PROBLEM_H: f32 = 24.0;
const PLACEHOLDER_BG: u32 = 0xD0_202028;
const BUILDING_BORDER: u32 = 0xFF_8080B0;
const FAILED_BORDER: u32 = 0xFF_E05050;
const BUILDING_TEXT: u32 = 0xFF_B0B0D0;
const FAILED_TEXT: u32 = 0xFF_FF9090;
const PAD: f32 = 3.0;
const LINE: f32 = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
const GLYPH_H: f32 = yog_ui::text::LINE_H - 1.0;
const CHAR_W: f32 = yog_ui::text::CHAR_W;
const MIN_COLS: usize = 4;
/// What the message of a library still being built starts with.
const BUILDING: &str = "Building";

/// The box shown for a component that can't be drawn: its name and why.
pub fn placeholder(name: &str, message: &str, w: f32, h: f32) -> Vec<DrawOp> {
    let building = message.starts_with(BUILDING);
    let border = if building { BUILDING_BORDER } else { FAILED_BORDER };
    let mut ops = vec![
        DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: w,
            y2: h,
            color: PLACEHOLDER_BG,
        },
        DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: w,
            y2: 1.0,
            color: border,
        },
        DrawOp::Rect {
            x1: 0.0,
            y1: h - 1.0,
            x2: w,
            y2: h,
            color: border,
        },
        DrawOp::Rect {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: h,
            color: border,
        },
        DrawOp::Rect {
            x1: w - 1.0,
            y1: 0.0,
            x2: w,
            y2: h,
            color: border,
        },
    ];
    let cols = (((w - 2.0 * PAD) / CHAR_W).floor() as usize).max(MIN_COLS);
    let mut lines = vec![format!("<{name}>")];
    for l in message.lines() {
        let chars: Vec<char> = l.chars().collect();
        lines.extend(chars.chunks(cols).map(|c| c.iter().collect::<String>()));
    }
    for (i, line) in lines.iter().enumerate() {
        let y = PAD + i as f32 * LINE;
        if y + GLYPH_H > h {
            break;
        }
        let color = if i == 0 {
            yog_gfx::Color::WHITE.0
        } else if building {
            BUILDING_TEXT
        } else {
            FAILED_TEXT
        };
        ops.push(DrawOp::Text {
            text: line.clone(),
            font: yog_ids::game::DEFAULT_FONT.into(),
            x: PAD,
            y,
            color,
            shadow: false,
        });
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(uid: &str, kind: &str, attrs: &[(&str, &str)], children: Vec<YuiNode>) -> YuiNode {
        YuiNode {
            uid: uid.into(),
            kind: kind.into(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children,
        }
    }

    #[test]
    fn custom_elements_become_marked_spacers_with_placeholders() {
        let root = node(
            "s",
            "Screen",
            &[("width", "100"), ("height", "60")],
            vec![
                node(
                    "c",
                    "StatBar",
                    &[("x", "4"), ("y", "6"), ("w", "50"), ("value", "0.5")],
                    vec![],
                ),
                node("l", "Label", &[("text", "hi")], vec![]),
            ],
        );
        assert!(is_component("StatBar") && !is_component("Label") && !is_component("statBar"));
        let p = prepare(&root, None, 200.0, 100.0);
        let c = &p.root.children[0];
        assert_eq!(c.kind, "Spacer");
        assert_eq!(c.attr("w"), Some("50"));
        assert_eq!(c.attr("h"), Some("24"));
        assert_eq!(c.attr("bg"), Some("#01D00000"));
        assert!(c.attr("value").is_none());
        assert_eq!(
            p.slots[0].req.props,
            vec![("value".to_string(), "0.5".to_string())]
        );

        let mut ops = vec![
            DrawOp::Rect {
                x1: 4.0,
                y1: 6.0,
                x2: 54.0,
                y2: 30.0,
                color: marker(0),
            },
            DrawOp::ClearScissor,
        ];
        let rects = HashMap::from([("c".to_string(), [4.0, 6.0, 50.0, 24.0])]);
        splice(&mut ops, &p, &rects);
        assert!(matches!(
            ops[0],
            DrawOp::Scissor {
                x: 4,
                y: 6,
                w: 50,
                h: 24
            }
        ));
        assert!(ops.iter().any(
            |o| matches!(o, DrawOp::Text { text, x, .. } if text == "<StatBar>" && *x == 7.0)
        ));
    }

    /// The whole path — `yog designer-lib`, the host process, splicing — on a real mod:
    /// `YOG_DESIGNER_MOD=../../Yog-Modlist YOG_IDLE_DESIGNER_HOST=target/debug/yog-idle-tauri
    /// YOG_DESIGNER_PNG=out.png cargo test --lib designer::custom -- --ignored`
    #[test]
    #[ignore]
    fn draws_a_mods_components() {
        let (Ok(root), Ok(out)) = (
            std::env::var("YOG_DESIGNER_MOD"),
            std::env::var("YOG_DESIGNER_PNG"),
        ) else {
            return;
        };
        let root = std::fs::canonicalize(root).unwrap();
        let started = Instant::now();
        while status(&root).status == Status::Building || started.elapsed() < DEBOUNCE {
            assert!(
                started.elapsed() < Duration::from_secs(600),
                "build took too long"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        let snap = status(&root);
        assert_eq!(snap.status, Status::Ready, "{:?}", snap.error);
        eprintln!(
            "{}: {:?}",
            snap.mod_name,
            snap.components.iter().map(|c| &c.name).collect::<Vec<_>>()
        );
        let doc = node(
            "s",
            "Screen",
            &[("width", "160"), ("height", "70"), ("bg", "#E0101010")],
            vec![
                node(
                    "p",
                    "Pill",
                    &[("x", "6"), ("y", "6"), ("label", "Custom!")],
                    vec![],
                ),
                node(
                    "b",
                    "StatBar",
                    &[
                        ("x", "6"),
                        ("y", "24"),
                        ("w", "140"),
                        ("label", "Loaded"),
                        ("value", "0.7"),
                    ],
                    vec![],
                ),
                node(
                    "m",
                    "Missing",
                    &[("x", "6"), ("y", "44"), ("w", "140"), ("h", "22")],
                    vec![],
                ),
            ],
        );
        let args = super::super::RenderArgs {
            path: root.join("src/demo.yui").to_string_lossy().into_owned(),
            root: doc,
            screen_w: 170,
            screen_h: 80,
            scale: 4,
            preview: false,
        };
        let r = super::super::render(&args).unwrap();
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD
            .decode(r.image.trim_start_matches("data:image/png;base64,"))
            .unwrap();
        std::fs::write(out, png).unwrap();
        let pill = r.rects.iter().find(|n| n.uid == "p").unwrap();
        assert!(
            pill.rect[2] > 10.0 && pill.rect[3] < 20.0,
            "{:?}",
            pill.rect
        );
    }

    #[test]
    fn cargo_errors_are_picked_out() {
        let e = build_errors(
            "   Compiling x\nerror[E0425]: cannot find value `y`\n  --> src/lib.rs:3:5\n   |\nerror: could not compile `x`\n",
        );
        assert_eq!(e, "error[E0425]: cannot find value `y`\n--> src/lib.rs:3:5");
    }
}
