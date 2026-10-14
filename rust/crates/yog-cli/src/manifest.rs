//! `yog.toml` — a mod project's manifest, the only build file mod authors
//! write.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The manifest's file name, in the project root.
pub const MANIFEST: &str = "yog.toml";
/// Where `yog build` generates the cargo project, under the project root.
pub const BUILD_DIR: &str = ".yog-build";
/// Plain crates.io dependencies.
pub const DEPS_SECTION: &str = "[dependencies]";
/// Other Yog mods' exports crates.
pub const EXPORT_DEPS_SECTION: &str = "[dependencies.exports]";
/// Rust edition when `[mod] edition` is absent.
pub const DEFAULT_EDITION: &str = "2024";
/// License of a mod, and of its exports crate, when none is given.
pub const DEFAULT_LICENSE: &str = "MIT OR Apache-2.0";
/// `yog-api` version when crates.io can't be asked (offline, etc.).
const FALLBACK_API_VERSION: &str = "0.5";

/// The current directory, if it's a mod project (has a `yog.toml`).
pub fn project_root() -> Result<PathBuf, String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
    if root.join(MANIFEST).is_file() {
        Ok(root)
    } else {
        Err(format!("no {MANIFEST} found in the current directory"))
    }
}

/// Parsed content of a `yog.toml` project file.
#[derive(Debug)]
pub struct YogToml {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    /// Translation keys of `name`/`description` when yog.toml gives them
    /// as `@<Set>.<name>` resources (see `resolve_metadata`).
    pub name_key: Option<String>,
    pub description_key: Option<String>,
    /// The mod's icon, an image source (`icon = "@Icons.logo"` resolved).
    pub icon: Option<String>,
    pub authors: Vec<String>,
    pub license: String,
    /// License for the generated `yog publish exports` bindings crate.
    /// Defaults to "MIT OR Apache-2.0" regardless of the mod's own license
    /// (AGPL propagating onto every consumer's dependency tree would kill
    /// adoption of the exports crate) — set `[mod] exports_license = "..."`
    /// to opt into matching the mod's own license instead.
    pub exports_license: Option<String>,
    pub edition: Option<String>,
    /// Optional: path to yog-api for local/monorepo development.
    /// Set via [dev] yog_api_path = "..."  or YOG_API_PATH env var.
    pub yog_api_path: Option<String>,
    /// Optional: pin yog-api version as "yog_api = \"X.Y\"" in [dependencies].
    pub yog_api_version: Option<String>,
    /// User-declared dependencies from [dependencies] section — plain
    /// crates.io crates, used exactly as written.
    pub dependencies: Vec<(String, String)>,
    /// Other Yog mods this mod depends on, from `[dependencies.exports]`.
    /// Each entry `name = spec` maps to that mod's generated exports crate:
    /// `{name} = { package = "{name}-exports", ...spec }` — so mod code
    /// imports it under its own name (`use {name}::...`), same as a direct
    /// dependency, and two mod dependencies never collide under a shared
    /// alias (unlike the old name-contains-hyphen heuristic this replaced).
    pub mod_dependencies: Vec<(String, String)>,
    /// `[dev-dependencies]` — crates only `yog bench` benchmarks use.
    pub dev_dependencies: Vec<(String, String)>,
    /// Named dev-instance launch configs from `[run.<name>]` sections.
    pub run_configs: Vec<RunConfig>,
    /// Content of other mods (original Java mods, or Yog mods) this mod
    /// names, declared up front: `[external.<namespace>]` with `items`,
    /// `blocks`, `entities`, `recipes` (paths: `["cutting_board"]`).
    pub externals: Vec<External>,
}

/// One `[external.<namespace>]` section.
#[derive(Debug, Default, Clone)]
pub struct External {
    pub namespace: String,
    /// `kind` (`items`, `blocks`, `entities`, `recipes`) → paths.
    pub ids: Vec<(String, Vec<String>)>,
}

/// A `[run.<name>]` section: where to drop the built artifact and what to
/// launch afterwards, for a full `yog run <name>` dev-instance workflow.
#[derive(Debug, Default, Clone)]
pub struct RunConfig {
    pub name: String,
    /// Optional human-readable label (`name = "..."` inside `[run.X]`), for
    /// editors to show/edit instead of the raw dotted section-key id. Falls
    /// back to `name` (the id) when absent.
    pub display_name: Option<String>,
    /// Directory the built `.yog` gets copied into (e.g. an instance's `yog-mods/`).
    /// Relative paths resolve against the mod project root.
    pub export_dir: Option<String>,
    /// Executable to launch after export (e.g. `java`, a wrapper script, `./gradlew`).
    pub command: Option<String>,
    /// Arguments passed to `command`, in order.
    pub args: Vec<String>,
    /// Working directory for `command`. Relative paths resolve against the project root.
    pub cwd: Option<String>,
    /// Extra environment variables as `KEY=VALUE` pairs.
    pub env: Vec<(String, String)>,
}

impl YogToml {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        parse_yog_toml(&text)
    }

    /// Resolve where yog-api lives.  Priority:
    ///   1. YOG_API_PATH env var
    ///   2. [dev] yog_api_path in yog.toml
    ///   3. yog_api_version in [dependencies] (e.g. "0.5")
    ///   4. YOG_API_VERSION env var
    ///   5. latest from crates.io
    pub fn api_dep(&self) -> String {
        if let Ok(p) = std::env::var("YOG_API_PATH") {
            // Resolve to absolute so it works from any subdirectory
            let abs = PathBuf::from(&p)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&p));
            return format!("yog-api = {{ path = {:?} }}", abs.to_string_lossy());
        }
        if let Some(p) = &self.yog_api_path {
            return format!("yog-api = {{ path = {p:?} }}");
        }
        let version = {
            let env_version = std::env::var("YOG_API_VERSION").ok();
            self.yog_api_version
                .as_deref()
                .or(env_version.as_deref())
                .map(|v| v.to_owned())
                .unwrap_or_else(latest_yog_api_version)
        };
        format!("yog-api = {:?}", version)
    }
}

pub fn parse_yog_toml(text: &str) -> Result<YogToml, String> {
    let mut section = "";
    let mut id = None::<String>;
    let mut name = None::<String>;
    let mut version = None::<String>;
    let mut description = None::<String>;
    let mut icon = None::<String>;
    let mut authors: Vec<String> = Vec::new();
    let mut license = None::<String>;
    let mut exports_license = None::<String>;
    let mut edition = None::<String>;
    let mut yog_api_path = None::<String>;
    let mut dependencies: Vec<(String, String)> = Vec::new();
    let mut mod_dependencies: Vec<(String, String)> = Vec::new();
    let mut dev_dependencies: Vec<(String, String)> = Vec::new();
    let mut run_configs: HashMap<String, RunConfig> = HashMap::new();
    let mut externals: Vec<External> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']');
            continue;
        }
        match section {
            "mod" => {
                if let Some(v) = field(line, "id") {
                    id = Some(v);
                }
                if let Some(v) = field(line, "name") {
                    name = Some(v);
                }
                if let Some(v) = field(line, "version") {
                    version = Some(v);
                }
                if let Some(v) = field(line, "description") {
                    description = Some(v);
                }
                if let Some(v) = field(line, "icon") {
                    icon = Some(v);
                }
                if let Some(v) = field(line, "license") {
                    license = Some(v);
                }
                if let Some(v) = field(line, "exports_license") {
                    exports_license = Some(v);
                }
                if let Some(v) = field(line, "edition") {
                    edition = Some(v);
                }
                if line.trim_start().starts_with("authors") {
                    authors = parse_string_array(line);
                }
            }
            "dev" => {
                if let Some(v) = field(line, "yog_api_path") {
                    yog_api_path = Some(v);
                }
            }
            "dependencies" => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    dependencies.push((name, spec));
                }
            }
            "dependencies.exports" => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    mod_dependencies.push((name, spec));
                }
            }
            "dev-dependencies" => {
                if let Some((name, spec)) = parse_dep_line(line) {
                    dev_dependencies.push((name, spec));
                }
            }
            _ if section.starts_with("external.") => {
                let ns = &section["external.".len()..];
                if externals.last().is_none_or(|e| e.namespace != ns) {
                    externals.push(External { namespace: ns.to_string(), ids: Vec::new() });
                }
                if let Some((key, _)) = line.split_once('=')
                    && let Some(e) = externals.last_mut()
                {
                    e.ids.push((key.trim().to_string(), parse_string_array(line)));
                }
            }
            _ if section.starts_with("run.") => {
                let run_name = &section["run.".len()..];
                let cfg = run_configs
                    .entry(run_name.to_string())
                    .or_insert_with(|| RunConfig {
                        name: run_name.to_string(),
                        ..Default::default()
                    });
                if let Some(v) = field(line, "name") {
                    cfg.display_name = Some(v);
                }
                if let Some(v) = field(line, "export_dir") {
                    cfg.export_dir = Some(v);
                }
                if let Some(v) = field(line, "command") {
                    cfg.command = Some(v);
                }
                if let Some(v) = field(line, "cwd") {
                    cfg.cwd = Some(v);
                }
                if line.trim_start().starts_with("args") {
                    cfg.args = parse_string_array(line);
                }
                if line.trim_start().starts_with("env") {
                    cfg.env = parse_string_array(line)
                        .into_iter()
                        .filter_map(|kv| {
                            kv.split_once('=')
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                        })
                        .collect();
                }
            }
            _ => {}
        }
    }
    let run_configs: Vec<RunConfig> = run_configs.into_values().collect();

    let id = id.ok_or("yog.toml: missing [mod] id")?;
    // Extract yog_api_version / yog-api from dependencies if there
    let mut yog_api_version = None;
    let mut filtered_deps = Vec::new();
    for (name, spec) in dependencies {
        if matches!(name.as_str(), "yog_api_version" | "yog-api" | "yog_api") {
            yog_api_version = Some(spec.trim_matches('"').to_string());
        } else {
            filtered_deps.push((name, spec));
        }
    }
    Ok(YogToml {
        name: name.unwrap_or_else(|| id.clone()),
        version: version.unwrap_or_else(|| "0.1.0".into()),
        description: description.unwrap_or_default(),
        name_key: None,
        description_key: None,
        icon,
        authors,
        license: license.unwrap_or_else(|| DEFAULT_LICENSE.into()),
        exports_license,
        edition,
        yog_api_path,
        yog_api_version,
        dependencies: filtered_deps,
        mod_dependencies,
        dev_dependencies,
        run_configs,
        externals,
        id,
    })
}

/// Parse a dependency line like `foo = "1.2"` or `bar = { version = "1", features = ["x"] }`
fn parse_dep_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.starts_with('#') || trimmed.is_empty() || trimmed.starts_with('[') {
        return None;
    }
    let eq_pos = trimmed.find('=')?;
    let name = trimmed[..eq_pos].trim().to_string();
    let spec = trimmed[eq_pos + 1..].trim().to_string();
    Some((name, spec))
}

/// Parse `key = "value"` returning the unquoted value.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    Some(rest.trim_matches('"').to_string())
}

/// Parse `key = ["a", "b"]` into a Vec<String>.
fn parse_string_array(line: &str) -> Vec<String> {
    let inner = line
        .find('[')
        .and_then(|s| line.rfind(']').map(|e| &line[s + 1..e]));
    inner
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Cargo derives a lib crate name from the package `name` by turning `-` into
/// `_`, but keeps case — so a mod `id` like "My-Mod" becomes the invalid
/// `My_Mod` and rustc warns on every build. `id` is a free-form project
/// identifier (not required to be a valid Rust ident), so the generated
/// wrapper pins `[lib] name` to a proper snake_case form instead.
pub fn to_snake_name(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for c in id.chars() {
        if c == '-' || c == '_' {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else if c.is_uppercase() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// rkyv version embedded at compile time via `build.rs`.
pub fn workspace_rkyv_version() -> String {
    env!("RKYV_VERSION").to_string()
}

/// Fetch the latest non-yanked version of `yog-api` from crates.io.
pub fn latest_yog_api_version() -> String {
    match std::process::Command::new("cargo")
        .args(["search", "yog-api", "--limit", "1"])
        .output()
    {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .and_then(|l| {
                    let mut parts = l.split_whitespace();
                    parts.next(); // skip crate name (e.g. "yog-api")
                    if parts.next() == Some("=") {
                        // skip the "=" separator
                        parts.next().map(|v| v.trim_matches('"').to_owned())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| FALLBACK_API_VERSION.into())
        }
        _ => FALLBACK_API_VERSION.into(),
    }
}
