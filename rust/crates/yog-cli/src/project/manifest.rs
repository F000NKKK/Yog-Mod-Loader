//! `yog.toml` — a mod project's manifest, the only build file mod authors
//! write.

use std::path::{Path, PathBuf};

/// A mod's version when its `yog.toml` gives none.
const DEFAULT_VERSION: &str = "0.1.0";

/// The manifest's file name, in the project root.
pub use yog_ids::file::MANIFEST;
/// Where `yog build` generates the cargo project, under the project root.
/// Where a mod's code is: its Rust and its Yog Script.
pub const SOURCES_DIR: &str = "src";
/// Its Rust's root file: a mod without one is Yog Script alone.
pub const RUST_LIB: &str = "lib.rs";
/// Plain crates.io dependencies.
pub const DEPS_SECTION: &str = "[dependencies]";
/// Other Yog mods' exports crates.
pub const EXPORT_DEPS_SECTION: &str = "[dependencies.exports]";
/// Rust edition when `[mod] edition` is absent.
pub const DEFAULT_EDITION: &str = "2024";
/// License of a mod, and of its exports crate, when none is given.
pub const DEFAULT_LICENSE: &str = "MIT OR Apache-2.0";
/// `yog-api` version when crates.io can't be asked (offline, etc.).
const FALLBACK_API_VERSION: &str = "0.98";

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
    /// The mod's id (its namespace), name, version and description.
    pub id: String,
    /// See [`YogToml::id`].
    pub name: String,
    /// See [`YogToml::id`].
    pub version: String,
    /// See [`YogToml::id`].
    pub description: String,
    /// Translation keys of `name`/`description` when yog.toml gives them
    /// as `@<Set>.<name>` resources (see `resolve_metadata`).
    pub name_key: Option<String>,
    /// See [`YogToml::name_key`].
    pub description_key: Option<String>,
    /// The mod's icon, an image source (`icon = "@res:Icons.logo"` resolved).
    pub icon: Option<String>,
    /// Who made it.
    pub authors: Vec<String>,
    /// The mod's own license.
    pub license: String,
    /// License for the generated `yog publish exports` bindings crate.
    /// Defaults to "MIT OR Apache-2.0" regardless of the mod's own license
    /// (AGPL propagating onto every consumer's dependency tree would kill
    /// adoption of the exports crate) — set `[mod] exports_license = "..."`
    /// to opt into matching the mod's own license instead.
    pub exports_license: Option<String>,
    /// The Minecraft version whose vanilla content it names (`[mod] datapack`).
    pub(crate) datapack: Option<String>,
    /// The Rust edition its code is in (the default: the toolchain's).
    pub edition: Option<String>,
    /// Optional: path to yog-api for local/monorepo development.
    /// Set via [dev] yog_api_path = "..."  or YOG_API_PATH env var.
    pub yog_api_path: Option<String>,
    /// Optional: pin yog-api version as "yog_api = \"X.Y\"" in [dependencies].
    pub yog_api_version: Option<String>,
    /// User-declared dependencies from [dependencies] section — plain
    /// crates.io crates, used exactly as written.
    pub(crate) dependencies: Vec<(String, String)>,
    /// Other Yog mods this mod depends on, from `[dependencies.exports]`.
    /// Each entry `name = spec` maps to that mod's generated exports crate:
    /// `{name} = { package = "{name}-exports", ...spec }` — so mod code
    /// imports it under its own name (`use {name}::...`), same as a direct
    /// dependency, and two mod dependencies never collide under a shared
    /// alias.
    pub mod_dependencies: Vec<(String, String)>,
    /// `[dev-dependencies]` — crates only `yog bench` benchmarks use.
    pub(crate) dev_dependencies: Vec<(String, String)>,
    /// Content of other mods (original Java mods, or Yog mods) this mod
    /// names, declared up front: `[external.<namespace>]` with `items`,
    /// `blocks`, `entities`, `recipes` (paths: `["cutting_board"]`).
    pub(crate) externals: Vec<External>,
    /// It sends to the server (`[network] to_server`): built with yog-api's
    /// feature for it.
    pub to_server: bool,
    /// The language the tools default to for its code (`[mod] language`).
    pub language: yog_formats::manifest::Language,
}

/// One `[external.<namespace>]` section.
#[derive(Debug, Default, Clone)]
pub(crate) struct External {
    pub(crate) namespace: String,
    /// The ids of each kind it lists.
    pub(crate) ids: Vec<(RefKind, Vec<String>)>,
}

impl YogToml {
    /// The manifest at `path`; what's wrong with it, said.
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
    pub(crate) fn api_dep(&self) -> String {
        if let Ok(p) = std::env::var(yog_ids::env::API_PATH) {
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
            let env_version = std::env::var(yog_ids::env::API_VERSION).ok();
            self.yog_api_version
                .as_deref()
                .or(env_version.as_deref())
                .map(|v| v.to_owned())
                .unwrap_or_else(latest_yog_api_version)
        };
        format!("yog-api = {:?}", version)
    }

    /// yog-api's dependency line, with the features the manifest asks for.
    pub(crate) fn api_line(&self) -> String {
        let line = self.api_dep();
        match (self.to_server, line.split_once(" = ")) {
            (true, Some((name, spec))) => {
                let spec =
                    super::cargo_toml::with_feature(spec, yog_ids::rust_crate::API_TO_SERVER);
                format!("{name} = {spec}")
            }
            _ => line,
        }
    }
}

// ── Reading ─────────────────────────────────────────────────────────────────

use yog_data::{Text, Value};
use yog_formats::manifest::ManifestFile;
use yog_ids::rust_crate;

use crate::content::refs::RefKind;

/// `v` as TOML's inline syntax — how a dependency is written into Cargo.toml.
fn toml_inline(v: &Value) -> String {
    let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    match v {
        Value::Map(m) => {
            let fields: Vec<String> = m
                .iter()
                .map(|(k, v)| {
                    let key = if k
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    {
                        k.to_string()
                    } else {
                        quote(k)
                    };
                    format!("{key} = {}", toml_inline(v))
                })
                .collect();
            format!("{{ {} }}", fields.join(", "))
        }
        Value::List(items) => format!(
            "[{}]",
            items.iter().map(toml_inline).collect::<Vec<_>>().join(", ")
        ),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        other => quote(other.as_str().unwrap_or_default()),
    }
}

/// A text of `yog.toml` as `resolve_metadata` reads it: `@res:Set.key` or the words (`@@` for a leading `@`).
fn text_form(t: Text) -> Result<String, String> {
    match t {
        Text::Literal(s) => Ok(yog_data::typed::escape_at(&s)),
        Text::Res(r) => Ok(r.to_string()),
        Text::NameOf(_) | Text::Lang(_) => Err(
            "a mod's name and description are text or a `.yres` text (`@res:Strings.mod_name`)"
                .into(),
        ),
    }
}

/// Reads `yog.toml` (TOML, through Yog Data's schema: every problem with its line).
pub(crate) fn parse_yog_toml(text: &str) -> Result<YogToml, String> {
    let f = yog_data::schema::from_toml::<ManifestFile>(text)
        .map_err(|e| toml_problems(&e))?
        .value;
    let m = f.module;
    let deps = dependencies(f.dependencies.0);
    let name = m.name.map(text_form).transpose()?;
    let description = m.description.map(text_form).transpose()?;
    let icon = m.icon.map(|i| match i {
        yog_data::Image::Source(s) => s,
        yog_data::Image::Res(r) => r.to_string(),
    });
    Ok(YogToml {
        name: name.unwrap_or_else(|| m.id.clone()),
        version: m.version.unwrap_or_else(|| DEFAULT_VERSION.into()),
        description: description.unwrap_or_default(),
        name_key: None,
        description_key: None,
        icon,
        authors: m.authors,
        license: m.license.unwrap_or_else(|| DEFAULT_LICENSE.into()),
        exports_license: m.exports_license,
        datapack: m.datapack,
        edition: m.edition,
        yog_api_path: f.dev.and_then(|d| d.yog_api_path),
        yog_api_version: deps.yog_api_version,
        dependencies: deps.crates,
        mod_dependencies: deps.mods,
        dev_dependencies: f
            .dev_dependencies
            .0
            .into_iter()
            .map(|(k, v)| (k, toml_inline(&v)))
            .collect(),
        externals: f
            .external
            .0
            .into_iter()
            .map(|(namespace, e)| external(namespace, e))
            .collect(),
        to_server: f.network.is_some_and(|n| n.to_server),
        language: m.language,
        id: m.id,
    })
}

/// What `yog.toml` got wrong, a line each: `yog.toml:<line>: <path>: <message>`.
fn toml_problems(e: &yog_data::Errors) -> String {
    e.0.iter()
        .map(|l| l.clone().in_file(MANIFEST).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `[dependencies]` sorted out: crates, the other Yog mods (`exports`), and
/// the yog-api version pinned there.
struct Dependencies {
    crates: Vec<(String, String)>,
    mods: Vec<(String, String)>,
    yog_api_version: Option<String>,
}

fn dependencies(section: Vec<(String, Value)>) -> Dependencies {
    let mut deps = Dependencies {
        crates: Vec::new(),
        mods: Vec::new(),
        yog_api_version: None,
    };
    for (name, spec) in section {
        match (name.as_str(), &spec) {
            ("exports", Value::Map(exports)) => deps
                .mods
                .extend(exports.iter().map(|(k, v)| (k.to_string(), toml_inline(v)))),
            (n, _)
                if [
                    rust_crate::API,
                    rust_crate::API_RUST_NAME,
                    rust_crate::API_VERSION_KEY,
                ]
                .contains(&n) =>
            {
                deps.yog_api_version = Some(
                    spec.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| toml_inline(&spec)),
                )
            }
            _ => deps.crates.push((name, toml_inline(&spec))),
        }
    }
    deps
}

/// `[external.<namespace>]`: the other mod's ids it lists, by kind.
fn external(namespace: String, e: yog_formats::manifest::ExternalSection) -> External {
    let ids = [
        (RefKind::Item, e.items),
        (RefKind::Block, e.blocks),
        (RefKind::Entity, e.entities),
        (RefKind::Recipe, e.recipes),
    ]
    .into_iter()
    .filter(|(_, v)| !v.is_empty())
    .collect();
    External { namespace, ids }
}

/// Fetch the latest non-yanked version of `yog-api` from crates.io.
pub fn latest_yog_api_version() -> String {
    match std::process::Command::new("cargo")
        .args(["search", rust_crate::API, "--limit", "1"])
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Yog-VLSI's manifest, as written — what the TOML reader that replaces
    /// this one must read the same.
    const VLSI: &str = r#"
[mod]
id          = "yog-vlsi"
name        = "@res:Strings.mod_name"
version     = "0.3.0"
description = "@res:Strings.mod_description"
authors     = ["F000NK", "Yog Team"]
license     = "AGPL-3.0-only"
edition     = "2024"

[dependencies]
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
yog-api    = "0.93"
yog-kit    = "0.4"

[dependencies.exports]
yog-pipes = "0.5"

## a comment
[external.farmersdelight]
items  = ["cutting_board", "tomato"]
blocks = ["stove"]
"#;

    #[test]
    fn reads_a_real_manifest() {
        let t = parse_yog_toml(VLSI).unwrap();
        assert_eq!(
            (t.id.as_str(), t.name.as_str(), t.version.as_str()),
            ("yog-vlsi", "@res:Strings.mod_name", "0.3.0")
        );
        assert_eq!(t.authors, ["F000NK", "Yog Team"]);
        assert_eq!(t.license, "AGPL-3.0-only");
        assert_eq!(
            t.yog_api_version.as_deref(),
            Some("0.93"),
            "yog-api is taken out of the dependencies"
        );
        let deps: Vec<&str> = t.dependencies.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(deps, ["serde", "serde_json", "yog-kit"]);
        assert_eq!(
            t.mod_dependencies,
            [("yog-pipes".to_string(), "\"0.5\"".to_string())]
        );
        let ext = &t.externals[0];
        assert_eq!(ext.namespace, "farmersdelight");
        assert_eq!(
            ext.ids[0],
            (
                RefKind::Item,
                vec!["cutting_board".to_string(), "tomato".to_string()]
            )
        );
    }

    #[test]
    fn problems_say_where() {
        let e = parse_yog_toml(
            "[mod]\nid = \"m\"\nautors = [\"me\"]\n\n[external.create]\nitem = [\"wrench\"]\n",
        )
        .unwrap_err();
        assert_eq!(
            e.lines().collect::<Vec<_>>(),
            [
                "yog.toml:3:1: mod.autors: unknown field `autors` — did you mean `authors`?",
                "yog.toml:6:1: external.create.item: unknown field `item` — did you mean `items`?",
            ]
        );
        let t = parse_yog_toml("[mod]\nid = \"m\"\nname = \"@@me\"\n").unwrap();
        assert_eq!(
            t.name, "@@me",
            "a literal @ stays escaped for resolve_metadata"
        );
    }

    #[test]
    fn needs_an_id() {
        assert!(parse_yog_toml("[mod]\nname = \"x\"").is_err());
        let t = parse_yog_toml("[mod]\nid = \"m\"").unwrap();
        assert_eq!(
            (t.name.as_str(), t.version.as_str()),
            ("m", "0.1.0"),
            "defaults"
        );
    }
}
